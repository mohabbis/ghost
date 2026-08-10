import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { prisma } from "@ghost/core/db";
import type { WorkflowSteps } from "@ghost/core/schema/step";

/**
 * Separation of duties + admin-only approval on the approval gate.
 *
 * Ghost's central claim is human approval on sensitive actions. Without this,
 * "human approval" means "somebody with a login clicked yes" — including the
 * same person who started the run, or any MEMBER. Approving is now
 * OWNER/ADMIN only; rejecting stays open to every member.
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
  let owner: string;
  let admin: string;
  let member: string;
  const slug = `sod-${Date.now()}`;

  beforeAll(async () => {
    const org = await prisma.organization.create({ data: { name: "SoD", slug } });
    orgId = org.id;
    const a = await prisma.user.create({
      data: { email: `${slug}-owner@example.com`, memberships: { create: { orgId, role: "OWNER" } } },
    });
    const b = await prisma.user.create({
      data: { email: `${slug}-admin@example.com`, memberships: { create: { orgId, role: "ADMIN" } } },
    });
    const c = await prisma.user.create({
      data: { email: `${slug}-member@example.com`, memberships: { create: { orgId, role: "MEMBER" } } },
    });
    owner = a.id;
    admin = b.id;
    member = c.id;
  });

  afterAll(async () => {
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.user.deleteMany({ where: { id: { in: [owner, admin, member] } } });
  });

  beforeEach(() => {
    session.current = { user: { id: owner, orgId } };
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
    const runId = await gatedRun({ requireSeparateApprover: true, triggeredById: owner });

    const res = await decide(runId, "approve");
    expect(res.status).toBe(403);

    const approval = await prisma.approval.findFirstOrThrow({ where: { runId } });
    expect(approval.status).toBe("PENDING");
    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("AWAITING_APPROVAL");
    expect(await prisma.runEvent.count({ where: { runId } })).toBe(0);

    const audit = await prisma.auditEvent.findFirst({
      where: { orgId, action: "approval.self_approval_refused" },
      orderBy: { seq: "desc" },
    });
    expect(audit?.metadata).toMatchObject({ runId, stepIndex: 1 });
  });

  it("lets a different ADMIN approve the same gate", async () => {
    const runId = await gatedRun({ requireSeparateApprover: true, triggeredById: owner });

    session.current = { user: { id: admin, orgId } };
    const res = await decide(runId, "approve");

    expect(res.status).toBe(200);
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId } });
    expect(approval.status).toBe("APPROVED");
    expect(approval.resolvedById).toBe(admin);
  });

  it("refuses a MEMBER who tries to approve", async () => {
    const runId = await gatedRun({ requireSeparateApprover: false, triggeredById: owner });

    session.current = { user: { id: member, orgId } };
    const res = await decide(runId, "approve");

    expect(res.status).toBe(403);
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId } });
    expect(approval.status).toBe("PENDING");
  });

  it("still lets the triggerer REJECT their own run", async () => {
    const runId = await gatedRun({ requireSeparateApprover: true, triggeredById: owner });

    const res = await decide(runId, "reject");

    expect(res.status).toBe(200);
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId } });
    expect(approval.status).toBe("REJECTED");
  });

  it("still lets a MEMBER reject (halting is not authorizing)", async () => {
    const runId = await gatedRun({ requireSeparateApprover: true, triggeredById: owner });

    session.current = { user: { id: member, orgId } };
    const res = await decide(runId, "reject");

    expect(res.status).toBe(200);
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId } });
    expect(approval.status).toBe("REJECTED");
  });

  it("does not restrict self-approval when the SoD policy is off", async () => {
    const runId = await gatedRun({ requireSeparateApprover: false, triggeredById: owner });

    const res = await decide(runId, "approve");

    expect(res.status).toBe(200);
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId } });
    expect(approval.status).toBe("APPROVED");
  });

  it("does not lock out an agent-started run whose triggerer is null", async () => {
    const runId = await gatedRun({ requireSeparateApprover: true, triggeredById: null });

    const res = await decide(runId, "approve");

    expect(res.status).toBe(200);
  });
});
