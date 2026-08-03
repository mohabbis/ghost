import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { prisma } from "@ghost/core/db";
import type { WorkflowSteps } from "@ghost/core/schema/step";

/**
 * Separation of duties on the approval gate.
 *
 * Ghost's central claim is human approval on sensitive actions. Without this,
 * "human approval" means "somebody with a login clicked yes" — including the
 * same person who started the run. These tests pin the rule and, just as
 * importantly, pin what it deliberately does *not* restrict.
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

vi.mock("@/lib/queue", () => ({
  enqueueRunWorkflow: async () => undefined,
  enqueueCompensateRun: async () => undefined,
}));

const session = vi.hoisted(() => ({
  current: null as null | { user: { id: string; orgId: string } },
}));
vi.mock("@/auth", () => ({ auth: async () => session.current }));

const steps: WorkflowSteps = [
  { id: "nav", type: "navigate", url: "https://example.com" },
  { id: "submit", type: "click", selector: { role: "button", name: "Submit order" } },
];

describe.skipIf(!hasDb)("approval separation of duties (Postgres)", () => {
  let orgId: string;
  let triggerer: string;
  let colleague: string;
  const slug = `sod-${Date.now()}`;

  beforeAll(async () => {
    const org = await prisma.organization.create({ data: { name: "SoD", slug } });
    orgId = org.id;
    const a = await prisma.user.create({
      data: { email: `${slug}-a@example.com`, memberships: { create: { orgId, role: "OWNER" } } },
    });
    const b = await prisma.user.create({
      data: { email: `${slug}-b@example.com`, memberships: { create: { orgId, role: "MEMBER" } } },
    });
    triggerer = a.id;
    colleague = b.id;
  });

  afterAll(async () => {
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.user.deleteMany({ where: { id: { in: [triggerer, colleague] } } });
  });

  beforeEach(() => {
    session.current = { user: { id: triggerer, orgId } };
  });

  /** A workflow + a run halted at a gate, with the policy set as given. */
  async function gatedRun(opts: { requireSeparateApprover: boolean; triggeredById: string | null }) {
    const wf = await prisma.workflow.create({
      data: {
        orgId,
        name: `wf-${Math.random().toString(36).slice(2, 8)}`,
        requireSeparateApprover: opts.requireSeparateApprover,
        versions: { create: { version: 1, steps: steps as never } },
      },
      include: { versions: true },
    });
    const run = await prisma.run.create({
      data: {
        orgId,
        workflowVersionId: wf.versions[0]!.id,
        triggeredById: opts.triggeredById,
        status: "AWAITING_APPROVAL",
        cursor: 1,
      },
    });
    await prisma.approval.create({
      data: { runId: run.id, stepIndex: 1, reason: "Clicks a \"submit\" control" },
    });
    return run.id;
  }

  async function decide(runId: string, decision: "approve" | "reject") {
    const { POST } = await import("./[id]/approvals/[stepIndex]/route.js");
    return POST(
      new Request(`http://localhost/api/runs/${runId}/approvals/1`, {
        method: "POST",
        body: JSON.stringify({ decision }),
      }),
      { params: Promise.resolve({ id: runId, stepIndex: "1" }) },
    );
  }

  it("refuses the triggerer's own approval, and records the refusal", async () => {
    const runId = await gatedRun({ requireSeparateApprover: true, triggeredById: triggerer });

    const res = await decide(runId, "approve");
    expect(res.status).toBe(403);

    // The gate must still be closed — a refused approval that resolved anything
    // would be worse than no control at all.
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId } });
    expect(approval.status).toBe("PENDING");
    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("AWAITING_APPROVAL");
    // Nothing entered the run journal: no decision was made.
    expect(await prisma.runEvent.count({ where: { runId } })).toBe(0);

    // …but the attempt is on the record. A silent 403 would leave no evidence
    // the control did its job.
    const audit = await prisma.auditEvent.findFirst({
      where: { orgId, action: "approval.self_approval_refused" },
      orderBy: { seq: "desc" },
    });
    expect(audit?.metadata).toMatchObject({ runId, stepIndex: 1 });
  });

  it("lets a different member approve the same gate", async () => {
    const runId = await gatedRun({ requireSeparateApprover: true, triggeredById: triggerer });

    session.current = { user: { id: colleague, orgId } };
    const res = await decide(runId, "approve");

    expect(res.status).toBe(200);
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId } });
    expect(approval.status).toBe("APPROVED");
    expect(approval.resolvedById).toBe(colleague);
  });

  it("still lets the triggerer REJECT their own run", async () => {
    // Rejecting stops the action rather than authorizing it. Blocking it would
    // leave whoever started a runaway run unable to halt it.
    const runId = await gatedRun({ requireSeparateApprover: true, triggeredById: triggerer });

    const res = await decide(runId, "reject");

    expect(res.status).toBe(200);
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId } });
    expect(approval.status).toBe("REJECTED");
  });

  it("does not restrict anything when the policy is off", async () => {
    // The default, and what every existing workflow has: behaviour unchanged.
    const runId = await gatedRun({ requireSeparateApprover: false, triggeredById: triggerer });

    const res = await decide(runId, "approve");

    expect(res.status).toBe(200);
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId } });
    expect(approval.status).toBe("APPROVED");
  });

  it("does not lock out an agent-started run whose triggerer is null", async () => {
    // `triggeredById` is null for agent-started runs. Comparing null to a null
    // user id would match and make the run unapprovable by anyone.
    const runId = await gatedRun({ requireSeparateApprover: true, triggeredById: null });

    const res = await decide(runId, "approve");

    expect(res.status).toBe(200);
  });
});
