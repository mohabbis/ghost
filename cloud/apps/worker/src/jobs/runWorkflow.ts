import type { Job } from "bullmq";
import { prisma, Prisma } from "@ghost/core/db";
import { parseWorkflowSteps, type WorkflowStep } from "@ghost/core/schema/step";
import { appendAuditEvent, appendRunEvent, runChainHead } from "@ghost/core/audit-log";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import { verifyAuditChain } from "@ghost/core/audit";
import { runEventPayloadFromRow } from "@ghost/core/audit-log";
import { resolveStep, ExpressionError, type ExpressionScope } from "@ghost/core/expr";
import {
  seal,
  open as openSealed,
  sessionAad,
  sessionKeyFromEnv,
  hasSessionKey,
  digest,
} from "@ghost/core/crypto";
import type { RunWorkflowJob } from "@ghost/core/queue";
import { planNextAction, isNoopStep, type ApprovalState } from "../runtime/state-machine.js";
import {
  journalFromEvents,
  journalFromLegacyRunSteps,
  type RunJournal,
} from "@ghost/core/journal";
import { planRestore, type ExecutedRecord } from "../runtime/restore.js";
import { claimSlot, recordThrottle, releaseSlot } from "../runtime/slots.js";
import { classifyError } from "../runtime/errors.js";
import { effectiveRetry, effectiveTimeout, backoffFor } from "../runtime/policy.js";
import {
  BrowserSession,
  applyRestorePlan,
  captureSessionState,
  runStep,
  verifyStep,
} from "../browser/driver.js";
import {
  artifactStore,
  screenshotKey,
  restoreScreenshotKey,
  sessionStateKey,
  sessionPrefix,
} from "../storage/artifacts.js";

/**
 * Execute (or resume) a workflow run.
 *
 * Imperative shell around the pure state machine (`planNextAction`). Position
 * comes from folding the run journal, never from a stored cursor, so a step
 * with a recorded success cannot execute twice — which is what makes a crash
 * mid-run safe for a workflow containing a payment.
 *
 * Two independent mechanisms block double execution, deliberately:
 *   - the run **lease** blocks two workers running concurrently;
 *   - the **journal** blocks a redelivered job re-running completed steps.
 *
 * Every transition appends to the run's hash chain, and run boundaries anchor
 * that chain's head into the org-wide audit chain. Cancellation is checked
 * before each step, so the run stays interruptible.
 *
 * This function must NOT throw on a business failure — it records the failure
 * and returns. Only infrastructure errors propagate to BullMQ for a job retry,
 * which is safe precisely because re-entry is idempotent.
 */

const LEASE_MS = 60_000;
const TERMINAL = new Set(["SUCCEEDED", "FAILED", "CANCELED"]);

export async function runWorkflowJob(job: Job<RunWorkflowJob>): Promise<void> {
  const { runId } = job.data;

  const run = await prisma.run.findUnique({
    where: { id: runId },
    include: {
      workflowVersion: true,
      approvals: true,
      steps: true,
    },
  });
  if (!run) throw new Error(`run ${runId} not found`);

  // A throttled run's own re-check, fired five minutes later. If the run was
  // handed a slot in the meantime it has moved on — possibly to an approval
  // gate — and re-driving it here would set it back to RUNNING and append
  // another `run.resumed`, another `gate.opened` and another approval audit
  // entry, none of which any human asked for. It is only ever a poll of
  // "am I still waiting?", so it does nothing unless the answer is yes.
  if (job.data.throttleRecheck && run.status !== "QUEUED") return;

  if (TERMINAL.has(run.status) || run.status === "INCIDENT") {
    // A run can reach a terminal state without the engine ever seeing it again:
    // a rejected approval and a cancel of a QUEUED run are both decided in the
    // web app. Captured browser credentials must not outlive them, so this
    // guard is also the cleanup path — web routes enqueue a job purely to
    // trigger it.
    if (run.sessionKey) await purgeSession(runId);
    // Same for the concurrency slot: a run canceled from the web never reaches
    // the loop below, and the next run must not wait on a slot nobody holds.
    await releaseSlot(runId).catch(() => undefined);
    return;
  }

  // ---- Lease ------------------------------------------------------------
  // One worker owns a run at a time. A second job for the same run is a no-op
  // rather than an error: stray re-enqueues are normal (approval resume, BullMQ
  // stalled-job redelivery) and must not look like failures.
  const owner = `${process.env.HOSTNAME ?? "worker"}:${process.pid}:${job.id ?? "nojob"}`;
  const leased = await prisma.run.updateMany({
    where: {
      id: runId,
      status: { in: ["QUEUED", "RUNNING", "AWAITING_APPROVAL"] },
      OR: [{ leaseOwner: null }, { leaseExpiresAt: { lt: new Date() } }],
    },
    data: {
      leaseOwner: owner,
      leaseExpiresAt: new Date(Date.now() + LEASE_MS),
      attempt: { increment: 1 },
    },
  });
  if (leased.count !== 1) return;

  const heartbeat = setInterval(() => {
    void prisma.run
      .updateMany({
        where: { id: runId, leaseOwner: owner },
        data: { leaseExpiresAt: new Date(Date.now() + LEASE_MS) },
      })
      .catch(() => undefined);
  }, Math.floor(LEASE_MS / 3));

  const steps = parseWorkflowSteps(run.workflowVersion.steps);
  let session: BrowserSession | undefined;
  try {
    // ---- Admission -------------------------------------------------------
    // Airflow's `max_active_runs`: hold the run in QUEUED rather than adding it
    // to a crowd already working the customer's system. Runs *after* the lease
    // so only one worker per run asks, and the claim commits inside the same
    // locked transaction as the count — see runtime/slots.ts.
    const admission = await claimSlot({
      runId,
      workflowId: run.workflowVersion.workflowId,
      alreadyStarted: run.startedAt !== null,
    });
    if (admission.kind === "throttle") {
      await recordThrottle(runId, run.orgId, admission.cap, admission.holders);
      return;
    }

    // ---- Journal ---------------------------------------------------------
    const events = await prisma.runEvent.findMany({
      where: { runId },
      orderBy: { seq: "asc" },
    });

    // A journal you do not verify is a log, not a proof. The independently
    // persisted expected tail is essential: links between surviving rows cannot
    // reveal deletion of a complete suffix (including deletion of every row).
    const chain = verifyAuditChain(
      events.map((e) => ({
        prevHash: e.prevHash,
        hash: e.hash,
        payload: runEventPayloadFromRow(e),
      })),
      { seq: run.journalSeq, hash: run.journalHead },
    );
    if (!chain.intact) {
      await failRun(
        runId,
        run.orgId,
        run.triggeredById,
        run.cursor,
        `JOURNAL_TAMPERED: run journal broken or truncated at event ${chain.firstBreakIndex}`,
      );
      return;
    }

    const loaded =
      events.length > 0
        ? journalFromEvents(events)
        : journalFromLegacyRunSteps(run.steps.map((s) => ({ index: s.index, status: s.status })));

    // The loop advances the journal in memory as steps commit, so it never has
    // to re-read the events it just wrote.
    const completed = new Set(loaded.completed);
    const journal: RunJournal = { ...loaded, completed };

    // ---- Approvals -------------------------------------------------------
    // Forward execution only ever consults FORWARD-phase approvals. Approving
    // the *reversal* of step N must never be mistaken for approving step N.
    const forwardApprovals = run.approvals.filter((a) => a.phase === "FORWARD");
    const now = new Date();
    const expired = forwardApprovals.filter(
      (a) => a.status === "APPROVED" && a.expiresAt !== null && a.expiresAt < now,
    );
    const liveExpired = expired.find((a) => !journal.completed.has(a.stepIndex));
    if (liveExpired) {
      await raiseIncident(
        runId,
        run.orgId,
        run.triggeredById,
        liveExpired.stepIndex,
        `approval for step ${liveExpired.stepIndex} expired before it was used`,
      );
      return;
    }

    const approvals: ApprovalState = {
      approved: new Set(
        forwardApprovals
          .filter((a) => a.status === "APPROVED" && !expired.includes(a))
          .map((a) => a.stepIndex),
      ),
      rejected: new Set(
        forwardApprovals.filter((a) => a.status === "REJECTED").map((a) => a.stepIndex),
      ),
    };

    // ---- Start / resume --------------------------------------------------
    // `RUNNING` and `startedAt` were already written by `claimSlot`: the status
    // *is* the slot, so it has to be taken under the admission lock rather than
    // here, where two runs could both have passed the count.
    if (!run.startedAt) {
      await appendRunEvent(runId, { type: RUN_EVENT_TYPES.runStarted });
      await appendAuditEvent(run.orgId, run.triggeredById, {
        action: "run.started",
        entityType: "Run",
        entityId: runId,
      });
    } else {
      await appendRunEvent(runId, { type: RUN_EVENT_TYPES.runResumed });
    }

    // Values captured by earlier `extract` steps, referenced by later steps.
    const scope: ExpressionScope = { steps: { ...journal.outputs } };
    const mutableOutputs = scope.steps as Record<string, Record<string, string>>;

    for (;;) {
      const fresh = await prisma.run.findUnique({
        where: { id: runId },
        select: { status: true },
      });
      if (fresh?.status === "CANCELED") {
        // The web route may already have journalled and anchored this. Appending
        // a second `run.canceled` after that anchor would leave the sealed head
        // no longer matching the journal — which reads as tampering.
        const already = await prisma.runEvent.findFirst({
          where: { runId, type: RUN_EVENT_TYPES.runCanceled },
          select: { id: true },
        });
        if (!already) {
          await appendRunEvent(runId, { type: RUN_EVENT_TYPES.runCanceled });
          await anchorRunChain(runId, run.orgId, run.triggeredById, "run.canceled");
        }
        return;
      }

      const action = planNextAction(steps, journal, approvals, (step) =>
        resolveStep(step, scope),
      );

      if (action.kind === "done") {
        // Status and terminal journal event commit together. Marking the run
        // SUCCEEDED first meant a crash before the append left a finished run
        // with no terminal event and no anchor — and re-entry returns early at
        // the terminal guard, so it could never be repaired.
        await prisma.$transaction(async (tx) => {
          await tx.run.update({
            where: { id: runId },
            data: { status: "SUCCEEDED", endedAt: new Date(), cursor: steps.length },
          });
          await appendRunEvent(runId, { type: RUN_EVENT_TYPES.runSucceeded }, tx);
        });
        await anchorRunChain(runId, run.orgId, run.triggeredById, "run.succeeded");
        await purgeSession(runId);
        return;
      }

      if (action.kind === "failed") {
        // A step the journal records as failed is recoverable: the incident
        // flow (retry / skip) exists precisely for it, and resuming into a
        // terminal FAILED would lose that flow whenever the worker died between
        // committing `step.failed` and raising the incident. A rejected
        // approval or a broken expression is not recoverable — retrying cannot
        // change either — so those stay terminal.
        if (action.recoverable) {
          await raiseIncident(runId, run.orgId, run.triggeredById, action.index, action.reason);
        } else {
          await failRun(runId, run.orgId, run.triggeredById, action.index, action.reason);
        }
        return;
      }

      if (action.kind === "indeterminate") {
        // Upsert, not update: a run that halted at a gate never created a
        // RunStep row for the gated index, and the timeline must still be able
        // to show "we do not know whether this happened."
        await prisma.runStep.upsert({
          where: { runId_index: { runId, index: action.index } },
          create: {
            runId,
            index: action.index,
            type: action.step.type,
            status: "UNKNOWN",
            label: action.step.label ?? action.step.type,
            error: action.reason,
            endedAt: new Date(),
          },
          update: { status: "UNKNOWN", endedAt: new Date(), error: action.reason },
        });
        await appendRunEvent(runId, {
          type: RUN_EVENT_TYPES.stepOutcomeUnknown,
          stepIndex: action.index,
          payload: { reason: action.reason },
        });
        await raiseIncident(
          runId,
          run.orgId,
          run.triggeredById,
          action.index,
          `OUTCOME_UNKNOWN: ${action.reason}`,
        );
        return;
      }

      if (action.kind === "gate") {
        // Capture browser state BEFORE closing Chromium, so the resume can
        // rebuild the page without replaying any prior action.
        let sessionData: { key: string; url: string; sha256: string } | null = null;
        if (session && hasSessionKey()) {
          sessionData = await captureSession(runId, action.index, session).catch(() => null);
        }

        await prisma.$transaction(async (tx) => {
          await tx.approval.upsert({
            where: {
              runId_stepIndex_phase: { runId, stepIndex: action.index, phase: "FORWARD" },
            },
            create: {
              runId,
              stepIndex: action.index,
              phase: "FORWARD",
              reason: action.reason,
            },
            update: {},
          });
          await tx.run.update({
            where: { id: runId },
            data: {
              status: "AWAITING_APPROVAL",
              cursor: action.index,
              ...(sessionData ? { sessionKey: sessionData.key, sessionUrl: sessionData.url } : {}),
            },
          });
          if (sessionData) {
            await appendRunEvent(
              runId,
              {
                type: RUN_EVENT_TYPES.sessionCaptured,
                stepIndex: action.index,
                // The blob's digest, never its contents — the chain commits to
                // something provable rather than readable.
                payload: { key: sessionData.key, sha256: sessionData.sha256, url: sessionData.url },
              },
              tx,
            );
          }
          await appendRunEvent(
            runId,
            {
              type: RUN_EVENT_TYPES.gateOpened,
              stepIndex: action.index,
              payload: { reason: action.reason },
            },
            tx,
          );
        });

        await appendAuditEvent(run.orgId, run.triggeredById, {
          action: "run.awaiting_approval",
          entityType: "Approval",
          entityId: runId,
          metadata: { stepIndex: action.index, reason: action.reason },
        });
        return;
      }

      // ---- execute --------------------------------------------------------
      const { index, step } = action;

      await prisma.runStep.upsert({
        where: { runId_index: { runId, index } },
        create: {
          runId,
          index,
          type: step.type,
          status: "RUNNING",
          label: step.label ?? step.type,
          startedAt: new Date(),
        },
        update: { status: "RUNNING", startedAt: new Date() },
      });

      const outcome = await executeStep({
        runId,
        orgId: run.orgId,
        actorId: run.triggeredById,
        index,
        step,
        steps,
        journal,
        ensureSession: async () => {
          if (!session) session = await openSession(runId, run.sessionKey);
          return session;
        },
        restoreIfNeeded: async (s) => restoreIfNeeded(runId, run, steps, journal, s),
      });

      if (outcome.kind === "halt") return;

      completed.add(index);
      if (outcome.outputs && Object.keys(outcome.outputs).length > 0) {
        mutableOutputs[step.id] = { ...mutableOutputs[step.id], ...outcome.outputs };
      }
    }
  } finally {
    clearInterval(heartbeat);
    if (session) await session.close().catch(() => undefined);
    await prisma.run
      .updateMany({
        where: { id: runId, leaseOwner: owner },
        data: { leaseOwner: null, leaseExpiresAt: null },
      })
      .catch(() => undefined);
    // Hands the slot on if the run reached a status that gives it up. A no-op
    // for a run parked at its gate (it keeps its slot) and for a throttled run
    // (it never took one — and releasing there would nominate itself, with no
    // delay, forever). Never allowed to mask the run's own outcome.
    await releaseSlot(runId).catch(() => undefined);
  }
}

// ---------------------------------------------------------------------------
// Step execution, with the timeout/retry policy applied.
// ---------------------------------------------------------------------------

interface ExecuteArgs {
  runId: string;
  orgId: string;
  actorId: string | null;
  index: number;
  step: WorkflowStep;
  steps: readonly WorkflowStep[];
  journal: RunJournal;
  ensureSession: () => Promise<BrowserSession>;
  restoreIfNeeded: (session: BrowserSession) => Promise<{ ok: true } | { ok: false; reason: string }>;
}

type StepOutcome =
  | { kind: "advance"; outputs: Record<string, string> }
  /** The run has already been put into a terminal or waiting state. */
  | { kind: "halt" };

async function executeStep(args: ExecuteArgs): Promise<StepOutcome> {
  const { runId, orgId, actorId, index, step } = args;
  const timeoutMs = effectiveTimeout(step);
  const retry = effectiveRetry(step);

  // Attempts already spent on this step in earlier entries of the job. Without
  // this the budget resets on every worker restart, so a `maxAttempts: 5` step
  // could run far more than five times across redeliveries.
  let attempt = args.journal.attempts.get(index) ?? 0;
  // The budget is the TOTAL across resumes, not a fresh allowance per entry —
  // adding to the prior count would reproduce the very leak this fixes. A
  // human-requested incident retry resets the count in the journal fold, which
  // is the one case where a fresh budget is intended.
  const budget = retry.maxAttempts;
  let lastError = "";

  while (attempt < budget) {
    attempt += 1;

    const delay = backoffFor(retry, attempt);
    if (delay > 0) await sleep(delay);

    // Recorded BEFORE the effect, so a crash leaves the step visibly in-flight
    // and the state machine can refuse to guess whether it landed.
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.stepStarted,
      stepIndex: index,
      payload: { type: step.type, attempt },
    });

    try {
      let screenshotRef: string | null = null;
      let verification: { passed: boolean; detail: string } | null = null;
      let outputs: Record<string, string> = {};
      let url: string | null = null;

      if (!isNoopStep(step)) {
        const session = await args.ensureSession();

        const restored = await args.restoreIfNeeded(session);
        if (!restored.ok) {
          // Recoverable by a human (restart, or skip the step), so this is an
          // incident rather than a dead run.
          await raiseIncident(
            runId,
            orgId,
            actorId,
            index,
            `RESTORE_UNSAFE: ${restored.reason}`,
          );
          return { kind: "halt" };
        }

        const result = await runStep(session.page, step, { timeoutMs });
        screenshotRef = await artifactStore().put(
          screenshotKey(runId, index),
          result.screenshot,
          "image/png",
        );
        verification = result.verification;
        outputs = result.outputs;
        url = result.url;

        // A failed verification is not a thrown action. Re-run the ASSERTION
        // only — never the action that preceded it. Re-clicking "Submit" to
        // find out whether the first click worked is the double-send.
        if (verification && !verification.passed) {
          const verifyPolicy = step.retry ?? { maxAttempts: 1, backoffMs: 1_000, factor: 2 };
          for (let vAttempt = 2; vAttempt <= verifyPolicy.maxAttempts; vAttempt++) {
            await sleep(backoffFor(verifyPolicy, vAttempt));
            await appendRunEvent(runId, {
              type: RUN_EVENT_TYPES.stepRetried,
              stepIndex: index,
              payload: { phase: "verify", attempt: vAttempt },
            });
            verification = await verifyStep(session.page, step);
            if (!verification || verification.passed) break;
          }
        }
      }

      const passed = verification ? verification.passed : true;

      await prisma.$transaction(async (tx) => {
        await tx.runStep.update({
          where: { runId_index: { runId, index } },
          data: {
            status: passed ? "SUCCEEDED" : "FAILED",
            screenshotKey: screenshotRef,
            verification: verification
              ? (verification as Prisma.InputJsonValue)
              : Prisma.JsonNull,
            output: Object.keys(outputs).length > 0 ? (outputs as Prisma.InputJsonValue) : Prisma.JsonNull,
            attempt,
            endedAt: new Date(),
          },
        });
        if (passed) {
          // Cursor, step row, and journal entry commit together. Persisting the
          // cursor per step is the direct fix for the crash-resume bug; deriving
          // position from the journal is what makes it structural.
          await tx.run.update({ where: { id: runId }, data: { cursor: index + 1 } });
          await appendRunEvent(
            runId,
            {
              type: RUN_EVENT_TYPES.stepSucceeded,
              stepIndex: index,
              payload: {
                type: step.type,
                stepId: step.id,
                url,
                screenshotKey: screenshotRef,
                ...(Object.keys(outputs).length > 0 ? { outputs } : {}),
              },
            },
            tx,
          );
        } else {
          await appendRunEvent(
            runId,
            {
              type: RUN_EVENT_TYPES.stepFailed,
              stepIndex: index,
              payload: { type: step.type, reason: "verification_failed", detail: verification?.detail },
            },
            tx,
          );
        }
      });

      await appendAuditEvent(orgId, actorId, {
        action: passed ? "step.succeeded" : "step.failed",
        entityType: "RunStep",
        entityId: `${runId}:${index}`,
        metadata: { type: step.type },
      });

      if (!passed) {
        await raiseIncident(runId, orgId, actorId, index, `verification failed at step ${index}`);
        return { kind: "halt" };
      }

      return { kind: "advance", outputs };
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      lastError = message;

      const errorClass =
        err instanceof ExpressionError ? "terminal" : classifyError(err, step);

      if (errorClass === "indeterminate") {
        await prisma.runStep.update({
          where: { runId_index: { runId, index } },
          data: { status: "UNKNOWN", error: message, attempt, endedAt: new Date() },
        });
        await appendRunEvent(runId, {
          type: RUN_EVENT_TYPES.stepOutcomeUnknown,
          stepIndex: index,
          payload: { error: message },
        });
        await raiseIncident(
          runId,
          orgId,
          actorId,
          index,
          `OUTCOME_UNKNOWN: step ${index} may or may not have taken effect — ${message}`,
        );
        return { kind: "halt" };
      }

      const canRetry = errorClass === "retryable" && attempt < budget;
      if (canRetry) {
        await appendRunEvent(runId, {
          type: RUN_EVENT_TYPES.stepRetried,
          stepIndex: index,
          payload: { phase: "action", attempt, error: message },
        });
        continue;
      }

      await prisma.runStep.update({
        where: { runId_index: { runId, index } },
        data: { status: "FAILED", error: message, attempt, endedAt: new Date() },
      });
      await appendRunEvent(runId, {
        type: RUN_EVENT_TYPES.stepFailed,
        stepIndex: index,
        payload: { type: step.type, error: message },
      });
      await appendAuditEvent(orgId, actorId, {
        action: "step.failed",
        entityType: "RunStep",
        entityId: `${runId}:${index}`,
        metadata: { error: message },
      });
      await raiseIncident(runId, orgId, actorId, index, message);
      return { kind: "halt" };
    }
  }

  await raiseIncident(runId, orgId, actorId, index, lastError || "step exhausted its retries");
  return { kind: "halt" };
}

// ---------------------------------------------------------------------------
// Session capture / restore
// ---------------------------------------------------------------------------

async function openSession(runId: string, sessionKey: string | null): Promise<BrowserSession> {
  if (!sessionKey || !hasSessionKey()) return BrowserSession.launch();
  try {
    const sealed = await artifactStore().get(sessionKey);

    // Check the blob against the digest the journal recorded when it was
    // captured, before decrypting it. Encryption alone proves the bytes are
    // authentic under the worker key; every run shares that key, so it does not
    // prove they are *this* run's bytes. The digest and the AAD below are what
    // make that true.
    const expected = await expectedSessionDigest(runId, sessionKey);
    if (expected && digest(sealed) !== expected) {
      throw new Error("captured session blob does not match its recorded digest");
    }

    const plaintext = openSealed(sealed, sessionKeyFromEnv(), sessionAad(runId, sessionKey));
    return await BrowserSession.launch(plaintext);
  } catch {
    // A missing, mismatched or undecryptable blob is not fatal by itself — the
    // restore planner still decides whether continuing without it is safe, and
    // it refuses rather than replaying actions when it is not.
    return BrowserSession.launch();
  }
}

/** The SHA-256 the journal recorded for this run's session blob, if any. */
async function expectedSessionDigest(runId: string, key: string): Promise<string | null> {
  const rows = await prisma.runEvent.findMany({
    where: { runId, type: RUN_EVENT_TYPES.sessionCaptured },
    orderBy: { seq: "desc" },
    select: { payload: true },
  });
  for (const row of rows) {
    const p = row.payload as { key?: unknown; sha256?: unknown } | null;
    if (p && p.key === key && typeof p.sha256 === "string") return p.sha256;
  }
  return null;
}

async function captureSession(
  runId: string,
  stepIndex: number,
  session: BrowserSession,
): Promise<{ key: string; url: string; sha256: string }> {
  const { state, url } = await captureSessionState(session.context, session.page);
  const key = sessionStateKey(runId, stepIndex);
  // AAD binds the ciphertext to this run and this exact key, so a blob cannot
  // be replayed into another run or another gate.
  const sealed = seal(state, sessionKeyFromEnv(), sessionAad(runId, key));
  await artifactStore().put(key, sealed, "application/octet-stream");
  return { key, url, sha256: digest(sealed) };
}

/** A session blob's useful life is one approval wait; keeping it is liability. */
async function purgeSession(runId: string): Promise<void> {
  await artifactStore().deletePrefix(sessionPrefix(runId)).catch(() => undefined);
  await prisma.run
    .update({ where: { id: runId }, data: { sessionKey: null, sessionUrl: null } })
    .catch(() => undefined);
}

/** Restoration happens once per browser session, not once per step. */
const restoreDone = new WeakSet<BrowserSession>();

async function restoreIfNeeded(
  runId: string,
  run: { sessionUrl: string | null },
  steps: readonly WorkflowStep[],
  journal: RunJournal,
  session: BrowserSession,
): Promise<{ ok: true } | { ok: false; reason: string }> {
  if (restoreDone.has(session)) return { ok: true };

  if (journal.completed.size === 0) {
    restoreDone.add(session);
    return { ok: true };
  }

  const rows = await prisma.runEvent.findMany({
    where: { runId, type: RUN_EVENT_TYPES.stepSucceeded },
    orderBy: { seq: "asc" },
  });
  const executed: ExecutedRecord[] = rows.map((r) => ({
    index: r.stepIndex ?? -1,
    url: readUrl(r.payload),
  }));

  // The journal says steps ran, but there are no per-step records to tell us
  // where. This is what a run predating the journal looks like. "Nothing has
  // executed" would be a lie that starts the page clean and silently redoes
  // work, so refuse instead and let a human restart it.
  if (executed.length === 0) {
    const reason =
      "this run completed steps before the execution journal existed, so the page " +
      "state it left behind cannot be reconstructed; restart it rather than replaying";
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.sessionRestoreFailed,
      payload: { reason },
    });
    return { ok: false, reason };
  }

  const plan = planRestore(steps, executed, run.sessionUrl);

  if (plan.kind === "cold") {
    restoreDone.add(session);
    return { ok: true };
  }

  if (plan.kind === "unsafe") {
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.sessionRestoreFailed,
      stepIndex: plan.blockingIndex,
      payload: { reason: plan.reason },
    });
    return { ok: false, reason: plan.reason };
  }

  await applyRestorePlan(session.page, steps, plan, { timeoutMs: 30_000 });

  // Post-restore evidence: capture what the page actually looks like, so a
  // human reviewing an incident is not asked to trust a claim they cannot see.
  const shot = await session.page.screenshot().catch(() => null);
  const nextIndex = firstPending(steps.length, journal);
  if (shot && nextIndex !== null) {
    await artifactStore()
      .put(restoreScreenshotKey(runId, nextIndex), shot, "image/png")
      .catch(() => undefined);
  }

  await appendRunEvent(runId, {
    type: RUN_EVENT_TYPES.sessionRestored,
    payload: { url: plan.url, reapplied: plan.reapply },
  });
  // Marked only now. Setting this up front meant a restore that threw partway
  // was skipped on the retry, and the step then ran against a half-rebuilt
  // page — resolving its selector against whatever happened to be there.
  restoreDone.add(session);
  return { ok: true };
}

function firstPending(length: number, journal: RunJournal): number | null {
  for (let i = 0; i < length; i++) if (!journal.completed.has(i)) return i;
  return null;
}

function readUrl(payload: unknown): string | null {
  if (!payload || typeof payload !== "object") return null;
  const url = (payload as { url?: unknown }).url;
  return typeof url === "string" ? url : null;
}

// ---------------------------------------------------------------------------
// Terminal transitions
// ---------------------------------------------------------------------------

async function failRun(
  runId: string,
  orgId: string,
  actorId: string | null,
  index: number,
  error: string,
): Promise<void> {
  await prisma.run.update({
    where: { id: runId },
    data: { status: "FAILED", endedAt: new Date(), error, cursor: index },
  });
  await appendRunEvent(runId, {
    type: RUN_EVENT_TYPES.runFailed,
    stepIndex: index,
    payload: { error },
  });
  await anchorRunChain(runId, orgId, actorId, "run.failed", { error, stepIndex: index });
  // A failed run is not resuming, so its captured credentials have no further
  // use. A session blob's useful life is one approval wait; keeping it is
  // liability, whichever way the run ended.
  await purgeSession(runId);
}

/**
 * A failed or unresumable step stops the run and waits for a human, rather than
 * killing it. Borrowed from Camunda incidents: an ops person watching a
 * 40-invoice batch wants to fix the one broken step, not restart the batch.
 */
async function raiseIncident(
  runId: string,
  orgId: string,
  actorId: string | null,
  index: number,
  reason: string,
): Promise<void> {
  await prisma.run.update({
    where: { id: runId },
    data: { status: "INCIDENT", error: reason, cursor: index },
  });
  await appendAuditEvent(orgId, actorId, {
    action: "run.incident",
    entityType: "Run",
    entityId: runId,
    metadata: { stepIndex: index, reason, runChainHead: await runChainHead(runId) },
  });
}

/**
 * Commit the run journal's head hash into the org-wide chain.
 *
 * This is what makes "the audit log is the execution history" literally true
 * without merging the tables: tamper with any RunEvent and the recomputed head
 * no longer matches the value sealed here.
 */
async function anchorRunChain(
  runId: string,
  orgId: string,
  actorId: string | null,
  action: string,
  extra?: Record<string, unknown>,
): Promise<void> {
  const head = await runChainHead(runId);
  await appendAuditEvent(orgId, actorId, {
    action,
    entityType: "Run",
    entityId: runId,
    metadata: { ...extra, runChainHead: head },
  });
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
