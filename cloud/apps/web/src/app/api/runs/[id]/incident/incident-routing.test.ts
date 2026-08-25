import {
  afterAll,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { prisma } from "@ghost/core/db";
import type { WorkflowSteps } from "@ghost/core/schema/step";

/**
 * Exception routing on the incident route: assignment, and the acknowledgement
 * a risky retry now requires.
 *
 * The behaviour under test is narrow but load-bearing. `OUTCOME_UNKNOWN` means
 * "this step may already have taken effect" — the engine deliberately lets a
 * human retry it anyway (see journal.ts on clearing `inFlight`), because only a
 * person can check the target system. What it must not do is let that happen as
 * a one-click accident indistinguishable from retrying a network blip, or leave
 * no record that the warning was ever shown.
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

const enqueueRunWorkflow = vi.hoisted(() =>
  vi.fn(async (_data: unknown) => undefined),
);
vi.mock("@/lib/queue", () => ({
  enqueueRunWorkflow,
  enqueueCompensateRun: async () => undefined,
}));

const session = vi.hoisted(() => ({
  current: null as null | { user: { id: string; orgId: string } },
}));
vi.mock("@/auth", () => ({ auth: async () => session.current }));

const steps: WorkflowSteps = [
  { id: "nav", type: "navigate", url: "https://example.com" },
  {
    id: "pay",
    type: "click",
    selector: { role: "button", name: "Pay invoice" },
  },
];

describe.skipIf(!hasDb)("incident routing (Postgres)", () => {
  let orgId: string;
  let userId: string;
  let otherOrgId: string;
  let outsiderId: string;
  const slug = `exc-${Date.now()}`;

  beforeAll(async () => {
    const org = await prisma.organization.create({
      data: { name: "Exc", slug },
    });
    orgId = org.id;
    const user = await prisma.user.create({
      data: {
        email: `${slug}@example.com`,
        memberships: { create: { orgId, role: "OWNER" } },
      },
    });
    userId = user.id;

    // A second tenant, to prove assignment cannot cross an org boundary.
    const other = await prisma.organization.create({
      data: { name: "Other", slug: `${slug}-other` },
    });
    otherOrgId = other.id;
    const outsider = await prisma.user.create({
      data: {
        email: `${slug}-outsider@example.com`,
        memberships: { create: { orgId: otherOrgId, role: "OWNER" } },
      },
    });
    outsiderId = outsider.id;
  });

  afterAll(async () => {
    for (const id of [orgId, otherOrgId]) {
      await prisma.organization
        .delete({ where: { id } })
        .catch(() => undefined);
    }
    for (const id of [userId, outsiderId]) {
      await prisma.user.delete({ where: { id } }).catch(() => undefined);
    }
  });

  beforeEach(() => {
    session.current = { user: { id: userId, orgId } };
    enqueueRunWorkflow.mockClear();
  });

  /**
   * An INCIDENT run parked on step 1 (the click), with the recorded step status
   * controlling whether its effect is known.
   */
  async function incidentRun(opts: {
    stepStatus: "FAILED" | "UNKNOWN";
    error: string;
    incidentKind?: string;
  }) {
    const wf = await prisma.workflow.create({
      data: {
        orgId,
        name: `wf-${Math.random().toString(36).slice(2, 8)}`,
        versions: { create: { version: 1, steps: steps as never } },
      },
      include: { versions: true },
    });
    const run = await prisma.run.create({
      data: {
        orgId,
        workflowVersionId: wf.versions[0]!.id,
        status: "INCIDENT",
        cursor: 1,
        error: opts.error,
        incidentKind: opts.incidentKind,
        incidentRaisedAt: new Date(),
        triggeredById: userId,
        steps: {
          create: {
            index: 1,
            type: "click",
            status: opts.stepStatus,
            label: "Pay invoice",
            error: opts.error,
          },
        },
      },
    });
    return run.id;
  }

  async function post(runId: string, body: unknown) {
    const { POST } = await import("./route");
    return POST(
      new Request(`http://localhost/api/runs/${runId}/incident`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      }),
      { params: Promise.resolve({ id: runId }) },
    );
  }

  // ---- The acknowledgement gate ----------------------------------------

  it("refuses an unacknowledged retry when the step's effect may already have happened", async () => {
    const runId = await incidentRun({
      stepStatus: "UNKNOWN",
      error:
        "OUTCOME_UNKNOWN: step 1 may or may not have taken effect — socket hang up",
    });

    const res = await post(runId, { action: "retry" });
    expect(res.status).toBe(409);
    const body = (await res.json()) as {
      requiresAcknowledgement?: boolean;
      kind?: string;
    };
    expect(body.requiresAcknowledgement).toBe(true);
    expect(body.kind).toBe("OUTCOME_UNKNOWN");

    // The run must not have moved, and nothing may have been enqueued.
    const run = await prisma.run.findUnique({ where: { id: runId } });
    expect(run?.status).toBe("INCIDENT");
    expect(enqueueRunWorkflow).not.toHaveBeenCalled();
  });

  it("allows the same retry once the risk is acknowledged, and records that it was", async () => {
    const runId = await incidentRun({
      stepStatus: "UNKNOWN",
      error:
        "OUTCOME_UNKNOWN: step 1 may or may not have taken effect — socket hang up",
    });

    // The acknowledgement must name the incident it is for; a bare boolean is
    // refused. This test previously sent only the flag, which is precisely the
    // hole the identity binding closes.
    const before = await prisma.run.findUnique({ where: { id: runId } });
    const res = await post(runId, {
      action: "retry",
      acknowledgeDuplicateRisk: true,
      expectStepIndex: 1,
      expectIncidentRaisedAt: before!.incidentRaisedAt!.toISOString(),
    });
    expect(res.status).toBe(200);

    const run = await prisma.run.findUnique({ where: { id: runId } });
    expect(run?.status).toBe("QUEUED");
    expect(enqueueRunWorkflow).toHaveBeenCalledOnce();

    // The acknowledgement is in the run journal, so the decision is attributable
    // to a person and sealed into the hash chain like any other event.
    const event = await prisma.runEvent.findFirst({
      where: { runId, type: "step.retry_requested" },
      orderBy: { seq: "desc" },
    });
    const payload = event?.payload as {
      acknowledgedDuplicateRisk?: boolean;
      kind?: string;
    } | null;
    expect(payload?.acknowledgedDuplicateRisk).toBe(true);
    expect(payload?.kind).toBe("OUTCOME_UNKNOWN");

    // And in the org-wide audit log.
    const audit = await prisma.auditEvent.findFirst({
      where: { orgId, entityId: runId, action: "run.incident_retried" },
      orderBy: { createdAt: "desc" },
    });
    const meta = audit?.metadata as {
      acknowledgedDuplicateRisk?: boolean;
    } | null;
    expect(meta?.acknowledgedDuplicateRisk).toBe(true);
  });

  it("refuses a bare acknowledgement with no incident identity", async () => {
    const runId = await incidentRun({
      stepStatus: "UNKNOWN",
      error: "OUTCOME_UNKNOWN: step 1 may or may not have taken effect",
    });
    const res = await post(runId, { action: "retry", acknowledgeDuplicateRisk: true });
    expect(res.status).toBe(409);
    expect(enqueueRunWorkflow).not.toHaveBeenCalled();
  });

  it("does not demand acknowledgement for an ordinary failure", async () => {
    // A plain recorded FAILED on a transient network error: retry is the normal,
    // safe, one-click path and must stay that way. If this test ever needs an
    // acknowledgement, the gate has become noise and will be clicked through.
    const runId = await incidentRun({
      stepStatus: "FAILED",
      error: "net::ERR_CONNECTION_RESET at https://example.com",
      incidentKind: "TRANSIENT",
    });

    const res = await post(runId, { action: "retry" });
    expect(res.status).toBe(200);
    expect(enqueueRunWorkflow).toHaveBeenCalledOnce();

    const event = await prisma.runEvent.findFirst({
      where: { runId, type: "step.retry_requested" },
      orderBy: { seq: "desc" },
    });
    const payload = event?.payload as {
      acknowledgedDuplicateRisk?: boolean;
    } | null;
    // Absent, not false: the flag's presence in the chain is the evidence.
    expect(payload?.acknowledgedDuplicateRisk).toBeUndefined();
  });

  it("ignores a stale stored kind that would understate the risk", async () => {
    // The stored label says TRANSIENT, but the recorded step outcome says the
    // effect is unknown. The authoritative signal must win, or writing a wrong
    // `incidentKind` would be a way to bypass the gate.
    const runId = await incidentRun({
      stepStatus: "UNKNOWN",
      error: "Timeout 30000ms exceeded",
      incidentKind: "TRANSIENT",
    });

    const res = await post(runId, { action: "retry" });
    expect(res.status).toBe(409);
  });

  // ---- Resolution clears the queue -------------------------------------

  it("clears routing fields when the incident resolves", async () => {
    const runId = await incidentRun({
      stepStatus: "FAILED",
      error: "net::ERR_CONNECTION_RESET",
      incidentKind: "TRANSIENT",
    });
    await prisma.run.update({
      where: { id: runId },
      data: { incidentAssigneeId: userId },
    });

    await post(runId, { action: "retry" });

    const run = await prisma.run.findUnique({ where: { id: runId } });
    // A resolved exception must leave the queue, and must not stay "assigned" to
    // someone who has finished with it.
    expect(run?.incidentKind).toBeNull();
    expect(run?.incidentAssigneeId).toBeNull();
  });

  // ---- Assignment -------------------------------------------------------

  it("assigns an exception to a member and records it", async () => {
    const runId = await incidentRun({ stepStatus: "FAILED", error: "boom" });

    const res = await post(runId, { action: "assign", assigneeId: userId });
    expect(res.status).toBe(200);

    const run = await prisma.run.findUnique({ where: { id: runId } });
    expect(run?.incidentAssigneeId).toBe(userId);
    // Assignment resumes nothing.
    expect(run?.status).toBe("INCIDENT");
    expect(enqueueRunWorkflow).not.toHaveBeenCalled();

    const audit = await prisma.auditEvent.findFirst({
      where: { orgId, entityId: runId, action: "run.incident_assigned" },
    });
    expect(audit).not.toBeNull();
  });

  it("unassigns on a null assignee", async () => {
    const runId = await incidentRun({ stepStatus: "FAILED", error: "boom" });
    await post(runId, { action: "assign", assigneeId: userId });

    const res = await post(runId, { action: "assign", assigneeId: null });
    expect(res.status).toBe(200);
    const run = await prisma.run.findUnique({ where: { id: runId } });
    expect(run?.incidentAssigneeId).toBeNull();
  });

  it("refuses to assign an exception to a user outside the org", async () => {
    // Tenant isolation. Without the membership check this would accept any user
    // id, putting another org's user on this org's queue and confirming that
    // account exists.
    const runId = await incidentRun({ stepStatus: "FAILED", error: "boom" });

    const res = await post(runId, { action: "assign", assigneeId: outsiderId });
    expect(res.status).toBe(404);
    const run = await prisma.run.findUnique({ where: { id: runId } });
    expect(run?.incidentAssigneeId).toBeNull();
  });

  it("does not warn about duplicate effects for an indeterminate read", async () => {
    // A `verify` whose outcome is unknown is a read: repeating it costs nothing.
    // Demanding acknowledgement here would train operators to click through the
    // prompt that exists for payments.
    const wf = await prisma.workflow.create({
      data: {
        orgId,
        name: `wf-${Math.random().toString(36).slice(2, 8)}`,
        versions: {
          create: {
            version: 1,
            steps: [
              { id: "nav", type: "navigate", url: "https://example.com" },
              {
                id: "chk",
                type: "verify",
                assertion: { kind: "textPresent", expected: "Paid" },
              },
            ] as never,
          },
        },
      },
      include: { versions: true },
    });
    const run = await prisma.run.create({
      data: {
        orgId,
        workflowVersionId: wf.versions[0]!.id,
        status: "INCIDENT",
        cursor: 1,
        error: "OUTCOME_UNKNOWN: step 1 may or may not have taken effect",
        incidentKind: "OUTCOME_UNKNOWN",
        incidentRaisedAt: new Date(),
        triggeredById: userId,
        steps: {
          create: {
            index: 1,
            type: "verify",
            status: "UNKNOWN",
            label: "Paid?",
          },
        },
      },
    });

    const res = await post(run.id, { action: "retry" });
    expect(res.status).toBe(200);
  });

  it("clears incidentRaisedAt when the incident resolves", async () => {
    const runId = await incidentRun({
      stepStatus: "FAILED",
      error: "net::ERR_CONNECTION_RESET",
      incidentKind: "TRANSIENT",
    });
    const before = await prisma.run.findUnique({ where: { id: runId } });
    expect(before?.incidentRaisedAt).not.toBeNull();

    await post(runId, { action: "retry" });

    // Leaving INCIDENT clears the parked-since stamp, or the next incident on
    // this run would inherit an age from the previous one.
    const after = await prisma.run.findUnique({ where: { id: runId } });
    expect(after?.incidentRaisedAt).toBeNull();
  });

  it("refuses an acknowledgement raised for a different incident", async () => {
    // The confirmation must be about the incident the human actually read. If
    // someone else resolves it and the run parks on a new risky step, a stale
    // open dialog must not be able to acknowledge the new one.
    const runId = await incidentRun({
      stepStatus: "UNKNOWN",
      error: "OUTCOME_UNKNOWN: step 1 may or may not have taken effect",
    });

    const stale = await post(runId, {
      action: "retry",
      acknowledgeDuplicateRisk: true,
      expectStepIndex: 0, // the run is parked on step 1
    });
    expect(stale.status).toBe(409);
    const body = (await stale.json()) as { staleAcknowledgement?: boolean };
    expect(body.staleAcknowledgement).toBe(true);
    expect(enqueueRunWorkflow).not.toHaveBeenCalled();

    // The same call naming the incident actually on screen is accepted.
    const run = await prisma.run.findUnique({ where: { id: runId } });
    const ok = await post(runId, {
      action: "retry",
      acknowledgeDuplicateRisk: true,
      expectStepIndex: 1,
      expectIncidentRaisedAt: run!.incidentRaisedAt!.toISOString(),
    });
    expect(ok.status).toBe(200);
  });

  it("refuses an acknowledgement whose incident timestamp has moved on", async () => {
    const runId = await incidentRun({
      stepStatus: "UNKNOWN",
      error: "OUTCOME_UNKNOWN: step 1 may or may not have taken effect",
    });

    const res = await post(runId, {
      action: "retry",
      acknowledgeDuplicateRisk: true,
      expectStepIndex: 1,
      expectIncidentRaisedAt: new Date(Date.now() - 86_400_000).toISOString(),
    });
    expect(res.status).toBe(409);
    expect(enqueueRunWorkflow).not.toHaveBeenCalled();
  });

  it("records the retry audit in the same transaction as the state change", async () => {
    const runId = await incidentRun({
      stepStatus: "FAILED",
      error: "net::ERR_CONNECTION_RESET",
      incidentKind: "TRANSIENT",
    });
    await post(runId, { action: "retry" });

    // Both must exist together: a run that left INCIDENT with no org audit
    // record would let the reclaimer drive the retry unaccounted for.
    const run = await prisma.run.findUnique({ where: { id: runId } });
    expect(run?.status).toBe("QUEUED");
    const audit = await prisma.auditEvent.findFirst({
      where: { orgId, entityId: runId, action: "run.incident_retried" },
    });
    expect(audit).not.toBeNull();
  });

  it("still audits a skip", async () => {
    const runId = await incidentRun({ stepStatus: "FAILED", error: "boom" });
    // Step 1 is a click on "Pay invoice" — sensitive, so skip is refused. Use
    // step 0 (navigate) instead by parking the run there.
    await prisma.run.update({ where: { id: runId }, data: { cursor: 0 } });

    const res = await post(runId, { action: "skip" });
    expect(res.status).toBe(200);
    const audit = await prisma.auditEvent.findFirst({
      where: { orgId, entityId: runId, action: "run.incident_skipped" },
    });
    expect(audit).not.toBeNull();
  });

  it("rejects an unknown action", async () => {
    const runId = await incidentRun({ stepStatus: "FAILED", error: "boom" });
    const res = await post(runId, { action: "resolve" });
    expect(res.status).toBe(400);
  });
});
