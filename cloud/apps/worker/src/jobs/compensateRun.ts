import type { Job } from "bullmq";
import { prisma, type Prisma } from "@ghost/core/db";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import {
  appendAuditEvent,
  appendRunEvent,
  runChainHead,
  runEventPayloadFromRow,
} from "@ghost/core/audit-log";
import { verifyAuditChain } from "@ghost/core/audit";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import type { CompensateRunJob } from "@ghost/core/queue";
import { journalFromEvents, journalFromLegacyRunSteps } from "@ghost/core/journal";
import {
  planCompensation,
  actionAsStep,
  describeActions,
  type CompensationEntry,
} from "@ghost/core/compensate";
import { resolveStep, ExpressionError, type ExpressionScope } from "@ghost/core/expr";
import { BrowserSession, applyStep, verifyStep } from "../browser/driver.js";
import { artifactStore, compensationScreenshotKey } from "../storage/artifacts.js";
import { claimSlot, recordThrottle, releaseSlot } from "../runtime/slots.js";
import { classifyException, type ExceptionKind } from "@ghost/core/classifier/exception";

/**
 * Routing fields for an incident raised by a *failed reversal*.
 *
 * A reversal that stops is a new exception, and the run may already be carrying
 * `incidentKind` / `incidentAssigneeId` from whatever stopped it in the forward
 * direction. Without overwriting all three fields here, the failed undo inherits
 * that classification and owner — so "refusing to reverse a run whose journal is
 * broken" could land in the queue labelled as a changed selector, on the desk of
 * whoever happened to own the earlier problem, with a "parked since" time from
 * hours ago.
 *
 * The assignee is deliberately cleared rather than kept: a reversal failure is a
 * different problem from the one that person accepted, so it goes back to the
 * unassigned queue to be triaged on its own terms.
 *
 * No step is passed to the classifier. These reasons are Ghost's own prose about
 * the reversal itself, not a driver error about a step, and duplicate risk for
 * the one indeterminate case is asserted by passing `OUTCOME_UNKNOWN` directly
 * at the call site.
 */
function freshIncidentRouting(
  input: { kind: ExceptionKind } | { reason: string },
): { incidentKind: string; incidentAssigneeId: null; incidentRaisedAt: Date } {
  return {
    incidentKind: "kind" in input ? input.kind : classifyException({ reason: input.reason }).kind,
    incidentAssigneeId: null,
    incidentRaisedAt: new Date(),
  };
}

/**
 * Reverse a run's completed side effects — the BPMN saga, over the run journal.
 *
 * Camunda triggers compensation by invoking completed activities' handlers in
 * reverse. Ghost can do the same because the journal already records precisely
 * what completed, so reversal is a backwards walk over evidence rather than a
 * guess.
 *
 * Properties this job is built to preserve:
 *
 * **The evidence is verified before it is trusted.** The journal is a hash
 * chain; folding it without checking it would let a single altered row invent a
 * reversal for an action that never happened, or hide one that did.
 *
 * **Reversal is itself gated.** Undoing a submitted order is a mutating action.
 * Each reversal is classified, and a sensitive one halts for approval exactly
 * like a forward step — using a COMPENSATION-phase Approval so it cannot
 * collide with the forward gate on the same index.
 *
 * **A reversal in flight is recorded before it acts.** `step.compensation_started`
 * is written before the browser touches anything, so a worker killed mid-cancel
 * leaves a trace instead of letting the redelivered job cancel twice.
 *
 * **Nothing is silently skipped.** A completed side effect with no defined
 * reversal is journalled as `step.irreversible` and surfaced. An operator who
 * thinks a run was fully undone when the confirmation email is still out is
 * worse off than one who was told the truth.
 *
 * **The reversal is itself audited, to the right person.** Every attempt
 * appends to the same hash-chained journal, attributed to whoever requested or
 * approved the reversal rather than to whoever started the original run.
 *
 * Known limitation: reversals run in a fresh, unauthenticated browser context.
 * A compensation against a system that requires a login will land on a sign-in
 * page. Closing that means keeping a run's captured credentials alive past the
 * run's end, which is a credential-retention decision rather than a code fix —
 * see cloud/docs/CURSOR_HANDOFF.md.
 */
export async function compensateRunJob(job: Job<CompensateRunJob>): Promise<void> {
  const { runId } = job.data;

  const run = await prisma.run.findUnique({
    where: { id: runId },
    include: { workflowVersion: true, approvals: true, steps: true },
  });
  if (!run) throw new Error(`run ${runId} not found`);

  // Cancelled between the request and this job picking it up. The cancel route
  // already sealed the run, but a reversal that was part-way through has to say
  // so — otherwise a half-undone run is indistinguishable from an ordinary
  // cancellation, and an operator would reasonably assume nothing was reversed.
  if (run.status === "CANCELED") {
    await sealCanceledCompensation(runId, run.orgId, actorFor(job, run.triggeredById));
    await releaseSlot(runId).catch(() => undefined);
    return;
  }
  if (run.status !== "COMPENSATING") return;

  // A reversal occupies the customer's system exactly as a forward run does, so
  // it goes through the same admission. Without this the undo route could put a
  // compensation to work alongside a forward run already filling the cap —
  // two of Ghost's browsers mutating the same system at once, which is the one
  // thing the cap exists to prevent.
  const admission = await claimSlot({
    runId,
    workflowId: run.workflowVersion.workflowId,
    alreadyStarted: true,
    runningStatus: "COMPENSATING",
  });
  if (admission.kind === "throttle") {
    await recordThrottle(runId, run.orgId, admission.cap, admission.holders, "compensation");
    return;
  }

  const steps = parseWorkflowSteps(run.workflowVersion.steps);

  const journalRecord = await prisma.run.findUniqueOrThrow({
    where: { id: runId },
    select: { journalHead: true, events: { orderBy: { seq: "asc" } } },
  });
  const events = journalRecord.events;

  // A journal you do not verify is a log, not a proof — the same rule forward
  // execution follows. Here it decides which real-world effects get reversed,
  // so trusting a broken chain could cancel an order that was never placed.
  if (events.length > 0 || journalRecord.journalHead !== null) {
    const chain = verifyAuditChain(
      events.map((e) => ({ prevHash: e.prevHash, hash: e.hash, payload: runEventPayloadFromRow(e) })),
    );
    const actualHead = events.at(-1)?.hash ?? null;
    if (!chain.intact || actualHead !== journalRecord.journalHead) {
      await prisma.run.update({
        where: { id: runId },
        data: {
          status: "INCIDENT",
          error: !chain.intact
            ? `JOURNAL_TAMPERED: refusing to reverse a run whose journal is broken at event ${chain.firstBreakIndex}`
            : "JOURNAL_TAMPERED: refusing to reverse a run whose journal tail does not match its expected head",
          // A failed reversal is a fresh exception, not a continuation of
          // whatever stopped the run first. Re-route it from scratch: inheriting
          // the forward incident's kind and assignee would file "the journal is
          // broken" under, say, a changed selector, on the desk of whoever
          // happened to own that.
          // A broken journal is not a step failure and none of the nine kinds names
          // it. UNKNOWN routes it to a human for judgement rather than inventing a
          // classification, which is the fail-closed default by design.
          ...freshIncidentRouting({ kind: "UNKNOWN" }),
        },
      });
      await releaseSlot(runId).catch(() => undefined);
      return;
    }
  }

  const journal =
    events.length > 0
      ? journalFromEvents(events)
      : journalFromLegacyRunSteps(run.steps.map((s) => ({ index: s.index, status: s.status })));

  const plan = planCompensation(steps, journal);

  // A step that started and never reported an outcome may still be landing.
  // Reversing around it would break reverse order and could report a complete
  // undo while the last action is still being applied.
  if (plan.inFlight.length > 0) {
    await prisma.run.update({
      where: { id: runId },
      data: {
        status: "INCIDENT",
        error: `cannot reverse while step ${plan.inFlight[0]} is still in flight`,
        // Retrying the undo once the step lands really does work, which is what
        // TRANSIENT means here.
        ...freshIncidentRouting({ kind: "TRANSIENT" }),
      },
    });
    await releaseSlot(runId).catch(() => undefined);
    return;
  }

  // Where each step's reversal stands, by its *latest* event rather than by
  // whether a given event type appears anywhere.
  //
  // Set membership is not enough: retry a failed reversal and the historical
  // `step.compensation_failed` sits in the journal forever, so a set-based test
  // would let it mask the new attempt's start — and the redelivered job would
  // cancel the order a second time. Events arrive in `seq` order, so the last
  // one wins.
  const latest = new Map<number, string>();
  for (const e of events) {
    if (e.stepIndex === null) continue;
    if (
      e.type === RUN_EVENT_TYPES.stepCompensationStarted ||
      e.type === RUN_EVENT_TYPES.stepCompensated ||
      e.type === RUN_EVENT_TYPES.stepCompensationFailed
    ) {
      latest.set(e.stepIndex, e.type);
    }
  }

  // Reversals already done — this job is re-entrant for the same reason forward
  // execution is.
  const done = new Set(
    [...latest].filter(([, t]) => t === RUN_EVENT_TYPES.stepCompensated).map(([i]) => i),
  );

  // Reversals that began and never reported an outcome. The browser action may
  // or may not have landed, so the at-most-once rule applies exactly as it does
  // to a forward step: stop and let a human decide.
  const started = [...latest]
    .filter(([, t]) => t === RUN_EVENT_TYPES.stepCompensationStarted)
    .map(([i]) => i)
    .sort((a, b) => a - b);
  if (started.length > 0) {
    await prisma.run.update({
      where: { id: runId },
      data: {
        status: "INCIDENT",
        cursor: started[0]!,
        error: `reversal of step ${started[0]} started but never reported an outcome — check the target system before retrying`,
        // The reversal's own effect is indeterminate, which is the one kind that
        // carries duplicate risk.
        ...freshIncidentRouting({ kind: "OUTCOME_UNKNOWN" }),
      },
    });
    await appendAuditEvent(run.orgId, actorFor(job, run.triggeredById), {
      action: "run.compensation_indeterminate",
      entityType: "Run",
      entityId: runId,
      metadata: { stepIndex: started[0] },
    });
    await releaseSlot(runId).catch(() => undefined);
    return;
  }

  const approvals = run.approvals.filter((a) => a.phase === "COMPENSATION");
  const approved = new Set(
    approvals.filter((a) => a.status === "APPROVED").map((a) => a.stepIndex),
  );
  const rejected = new Set(
    approvals.filter((a) => a.status === "REJECTED").map((a) => a.stepIndex),
  );
  /** Who approved the reversal of a given step, when one did. */
  const approverOf = new Map(
    approvals.filter((a) => a.resolvedById).map((a) => [a.stepIndex, a.resolvedById!]),
  );

  const requester = actorFor(job, run.triggeredById);

  // Keyed on the start event itself, not on "no reversal has completed yet".
  // A plan whose first reversal gates records the start and returns before any
  // `step.compensated` exists, so the completed-count proxy makes the resumed
  // job announce the start a second time and repeat every irreversible entry.
  const alreadyAnnounced = events.some((e) => e.type === RUN_EVENT_TYPES.compensationStarted);
  if (!alreadyAnnounced) {
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.compensationStarted,
      payload: { reversible: plan.entries.length, irreversible: plan.irreversible.length },
    });
    // Record what cannot be undone up front, so the journal carries it even if
    // the reversal later fails partway.
    for (const entry of plan.irreversible) {
      await appendRunEvent(runId, {
        type: RUN_EVENT_TYPES.stepIrreversible,
        stepIndex: entry.stepIndex,
        payload: { stepId: entry.stepId, reason: entry.reason },
      });
    }
  }

  // Values captured by `extract` steps during the forward run. A reversal that
  // has to open the resource it created needs them — without resolution it
  // would navigate to the literal `{{ steps.create.id }}`.
  const scope: ExpressionScope = { steps: journal.outputs };

  let session: BrowserSession | undefined;

  try {
    for (const entry of plan.entries) {
      if (done.has(entry.stepIndex)) continue;

      // Checked before each reversal, exactly as the forward loop checks before
      // each step. A reversal already running finishes — stopping mid-action
      // would leave an effect nobody can account for — but the ones after it do
      // not start.
      const fresh = await prisma.run.findUnique({
        where: { id: runId },
        select: { status: true },
      });
      if (fresh?.status === "CANCELED") {
        await sealCanceledCompensation(runId, run.orgId, requester);
        return;
      }

      if (rejected.has(entry.stepIndex)) {
        await finish(
          runId,
          run.orgId,
          approverOf.get(entry.stepIndex) ?? requester,
          "rejected",
          plan.irreversible.length,
        );
        return;
      }

      // Gate before touching anything.
      if (entry.requiresApproval && !approved.has(entry.stepIndex)) {
        await openGate(runId, run.orgId, requester, entry);
        return;
      }

      if (!session) session = await BrowserSession.launch();
      const ok = await reverseOne({
        runId,
        orgId: run.orgId,
        actorId: approverOf.get(entry.stepIndex) ?? requester,
        entry,
        session,
        scope,
      });
      if (!ok) return;
      done.add(entry.stepIndex);
    }

    await finish(runId, run.orgId, requester, "complete", plan.irreversible.length);
  } finally {
    if (session) await session.close().catch(() => undefined);
    // COMPENSATING holds a concurrency slot like RUNNING does. Reaching
    // COMPENSATED or INCIDENT frees it, and the next run should not wait for
    // the safety-net re-check to notice.
    await releaseSlot(runId).catch(() => undefined);
  }
}


/**
 * Close out a reversal that was stopped by request.
 *
 * The run stays CANCELED — the cancel route already sealed that transition —
 * but a partly-reversed run must not read as an ordinary cancellation. The
 * counts come from the journal rather than from a loop variable, so this says
 * the same thing whether the stop was seen before the job started or between
 * two reversals.
 *
 * A no-op for a run whose reversal never began, and for one already sealed, so
 * a redelivered job cannot append a second ending.
 */
async function sealCanceledCompensation(
  runId: string,
  orgId: string,
  actorId: string | null,
): Promise<void> {
  const events = await prisma.runEvent.findMany({
    where: { runId },
    orderBy: { seq: "asc" },
    select: { type: true, stepIndex: true },
  });
  const began = events.some((e) => e.type === RUN_EVENT_TYPES.compensationStarted);
  const ended = events.some((e) => e.type === RUN_EVENT_TYPES.compensationFinished);
  if (!began || ended) return;

  const reversed = new Set(
    events
      .filter((e) => e.type === RUN_EVENT_TYPES.stepCompensated)
      .map((e) => e.stepIndex)
      .filter((i): i is number => i !== null),
  ).size;

  await prisma.$transaction(async (tx: Prisma.TransactionClient) => {
    await tx.run.update({
      where: { id: runId },
      data: {
        error: `reversal stopped by request after undoing ${reversed} step(s) — the rest of this run's effects are still in place`,
      },
    });
    await appendRunEvent(
      runId,
      { type: RUN_EVENT_TYPES.compensationFinished, payload: { outcome: "canceled", reversed } },
      tx,
    );
  });

  await appendAuditEvent(orgId, actorId, {
    action: "run.compensation_canceled",
    entityType: "Run",
    entityId: runId,
    metadata: { reversed, runChainHead: await runChainHead(runId) },
  });
}

/**
 * Whose name goes against the reversal.
 *
 * `Run.triggeredById` is the last resort, not the default: it names whoever
 * started the original run, which for an agent-triggered or colleague-triggered
 * run is the wrong person to record against a mutation they did not ask for.
 */
function actorFor(job: Job<CompensateRunJob>, fallback: string | null): string | null {
  return job.data.requestedById ?? fallback;
}

/**
 * Halt for approval, and seal the head before the wait.
 *
 * Compensation extends a journal that was already anchored when the run
 * terminated. Pausing without re-anchoring leaves `/api/audit/verify`
 * comparing a grown journal against the old terminal anchor for the length of
 * the approval wait, reporting a healthy compensation as tampered.
 */
async function openGate(
  runId: string,
  orgId: string,
  actorId: string | null,
  entry: CompensationEntry,
): Promise<void> {
  const actions = describeActions(entry.compensation);

  await prisma.$transaction(async (tx) => {
    await tx.approval.upsert({
      where: { runId_stepIndex_phase: { runId, stepIndex: entry.stepIndex, phase: "COMPENSATION" } },
      create: {
        runId,
        stepIndex: entry.stepIndex,
        phase: "COMPENSATION",
        reason: entry.approvalReason ?? entry.compensation.description,
      },
      update: {},
    });
    await tx.run.update({
      where: { id: runId },
      data: { status: "AWAITING_APPROVAL", cursor: entry.stepIndex },
    });
    await appendRunEvent(
      runId,
      {
        type: RUN_EVENT_TYPES.gateOpened,
        stepIndex: entry.stepIndex,
        payload: { phase: "COMPENSATION", reason: entry.approvalReason, actions },
      },
      tx,
    );
  });

  await appendAuditEvent(orgId, actorId, {
    action: "run.awaiting_approval",
    entityType: "Approval",
    entityId: runId,
    metadata: {
      stepIndex: entry.stepIndex,
      phase: "COMPENSATION",
      reason: entry.approvalReason,
      actions,
      runChainHead: await runChainHead(runId),
    },
  });
}

interface ReverseArgs {
  runId: string;
  orgId: string;
  actorId: string | null;
  entry: CompensationEntry;
  session: BrowserSession;
  scope: ExpressionScope;
}

async function reverseOne({
  runId,
  orgId,
  actorId,
  entry,
  session,
  scope,
}: ReverseArgs): Promise<boolean> {
  // Compensation actions carry no per-action policy of their own, so they use
  // the same default budget a step would.
  const timeoutMs = Number(process.env.GHOST_STEP_TIMEOUT_MS) || 30_000;

  // Written BEFORE the browser acts. If the worker dies between here and the
  // terminal event, re-entry finds a start with no outcome and stops rather
  // than cancelling the order a second time.
  await appendRunEvent(runId, {
    type: RUN_EVENT_TYPES.stepCompensationStarted,
    stepIndex: entry.stepIndex,
    payload: { stepId: entry.stepId, actions: describeActions(entry.compensation) },
  });

  // Has any reversal action landed, or been in flight, by the time we fail?
  //
  // This decides whether a failed reversal is safely re-runnable. Set *before*
  // `applyStep` so an action that timed out mid-flight counts: a reversal click
  // that timed out may well have reached the server, and pressing Undo again
  // would send it twice. Only a failure before the very first action starts —
  // an unresolvable expression on action 1 — leaves the reversal provably
  // untouched.
  let appliedOrInFlight = false;

  try {
    for (const action of entry.compensation.actions) {
      const step = resolveStep(actionAsStep(action, entry.stepId), scope);
      appliedOrInFlight = true;
      await applyStep(session.page, step, { timeoutMs });
    }

    const verification = entry.compensation.verify
      ? await verifyStep(session.page, {
          id: entry.stepId,
          type: "verify",
          assertion: entry.compensation.verify,
        })
      : null;

    if (verification && !verification.passed) {
      throw new Error(`reversal verification failed: ${verification.detail}`);
    }

    const shot = await session.page.screenshot().catch(() => null);
    // A compensation-specific key. Reusing the forward step's key overwrote the
    // screenshot of what originally happened — destroying the evidence the
    // reversal exists to account for.
    const key = shot
      ? await artifactStore()
          .put(compensationScreenshotKey(runId, entry.stepIndex), shot, "image/png")
          .catch(() => null)
      : null;

    await prisma.$transaction(async (tx) => {
      await tx.runStep.updateMany({
        where: { runId, index: entry.stepIndex },
        // COMPENSATED, not SKIPPED. The forward action ran; it was undone. A
        // timeline that says "skipped" is describing a run that never happened.
        data: { status: "COMPENSATED", endedAt: new Date() },
      });
      await appendRunEvent(
        runId,
        {
          type: RUN_EVENT_TYPES.stepCompensated,
          stepIndex: entry.stepIndex,
          payload: {
            stepId: entry.stepId,
            description: entry.compensation.description,
            screenshotKey: key,
          },
        },
        tx,
      );
    });

    await appendAuditEvent(orgId, actorId, {
      action: "step.compensated",
      entityType: "RunStep",
      entityId: `${runId}:${entry.stepIndex}`,
      metadata: { description: entry.compensation.description },
    });
    return true;
  } catch (err) {
    const message =
      err instanceof ExpressionError
        ? `reversal references a value this run never captured: ${err.message}`
        : err instanceof Error
          ? err.message
          : String(err);

    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.stepCompensationFailed,
      stepIndex: entry.stepIndex,
      payload: { stepId: entry.stepId, error: message },
    });
    // A failed reversal leaves the system in a half-undone state, which is
    // exactly when a human needs to look. Stop here rather than pressing on.
    await prisma.run.update({
      where: { id: runId },
      data: {
        status: "INCIDENT",
        cursor: entry.stepIndex,
        error: `reversal of step ${entry.stepIndex} failed: ${message}`,
        // Classifying this from the message alone read a timed-out reversal as
        // TRANSIENT — "just retry" — when a mutating reversal action that timed
        // out may already have taken effect. Pressing Undo again would then
        // re-execute it under the still-valid approval with no duplicate-risk
        // acknowledgement, which is the exact double-execution the gate exists
        // to prevent, reached from the reversal side.
        ...freshIncidentRouting(
          appliedOrInFlight ? { kind: "OUTCOME_UNKNOWN" } : { reason: message },
        ),
      },
    });
    await appendAuditEvent(orgId, actorId, {
      action: "run.compensation_failed",
      entityType: "Run",
      entityId: runId,
      metadata: { stepIndex: entry.stepIndex, error: message },
    });
    return false;
  }
}

async function finish(
  runId: string,
  orgId: string,
  actorId: string | null,
  outcome: "complete" | "rejected",
  irreversibleCount: number,
): Promise<void> {
  // Status and terminal journal event commit together. Writing the status first
  // meant a crash before the append left a terminal COMPENSATED run with an
  // incomplete, unanchored journal — and re-entry returns immediately for any
  // status other than COMPENSATING, so nothing could ever repair it.
  await prisma.$transaction(async (tx: Prisma.TransactionClient) => {
    await tx.run.update({
      where: { id: runId },
      data: {
        status: outcome === "complete" ? "COMPENSATED" : "INCIDENT",
        endedAt: new Date(),
        // Stated on the run itself, so nobody reads "COMPENSATED" as "fully
        // undone" when part of it could not be.
        error:
          outcome === "rejected"
            ? "reversal rejected by an approver"
            : irreversibleCount > 0
              ? `reversed, but ${irreversibleCount} completed step(s) had no defined reversal`
              : null,
      },
    });
    await appendRunEvent(
      runId,
      {
        type: RUN_EVENT_TYPES.compensationFinished,
        payload: { outcome, irreversible: irreversibleCount },
      },
      tx,
    );
  });

  await appendAuditEvent(orgId, actorId, {
    action: outcome === "complete" ? "run.compensated" : "run.compensation_rejected",
    entityType: "Run",
    entityId: runId,
    metadata: { irreversible: irreversibleCount, runChainHead: await runChainHead(runId) },
  });
}
