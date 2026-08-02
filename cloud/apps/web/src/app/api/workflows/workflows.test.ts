import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { prisma } from "@ghost/core/db";
import type { WorkflowSteps } from "@ghost/core/schema/step";

/**
 * Route tests for authoring workflows.
 *
 * These handlers are the authority on what reaches `WorkflowVersion.steps` —
 * the editor's validation is a convenience for whoever is typing, and a
 * hand-rolled POST bypasses it entirely. So the properties worth testing are
 * the ones a malicious or careless client could otherwise break: tenant
 * scoping, step validity, version monotonicity, and whether the audit trail
 * actually records what happened.
 *
 * Requires DATABASE_URL; skips cleanly without one so unit-only CI stays green.
 * Note that means CI does *not* run these — they must be run locally against a
 * migrated database before trusting a green PR page.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

// `auth()` is swapped per-test to impersonate an org. Everything below the
// route handler — Prisma, the audit chain — is real.
const session = vi.hoisted(() => ({ current: null as null | { user: { id: string; orgId: string } } }));
vi.mock("@/auth", () => ({ auth: async () => session.current }));

const steps: WorkflowSteps = [
  { id: "nav", type: "navigate", url: "https://example.com/orders" },
  {
    id: "fill",
    type: "fill",
    selector: { role: "textbox", name: "Full name" },
    value: "Ada Lovelace",
    sensitive: false,
  },
  { id: "submit", type: "click", selector: { role: "button", name: "Submit order" } },
];

describe.skipIf(!hasDb)("workflow authoring routes (Postgres)", () => {
  let orgA: string;
  let orgB: string;
  let userA: string;
  const slug = `authoring-${Date.now()}`;

  beforeAll(async () => {
    const a = await prisma.organization.create({ data: { name: "A", slug: `${slug}-a` } });
    const b = await prisma.organization.create({ data: { name: "B", slug: `${slug}-b` } });
    orgA = a.id;
    orgB = b.id;
    const u = await prisma.user.create({
      data: {
        email: `${slug}@example.com`,
        memberships: { create: { orgId: orgA, role: "OWNER" } },
      },
    });
    userA = u.id;
    session.current = { user: { id: userA, orgId: orgA } };
  });

  afterAll(async () => {
    await prisma.organization.deleteMany({ where: { id: { in: [orgA, orgB] } } });
    await prisma.user.deleteMany({ where: { id: userA } });
  });

  async function post(body: unknown) {
    const { POST } = await import("./route.js");
    return POST(new Request("http://localhost/api/workflows", { method: "POST", body: JSON.stringify(body) }));
  }

  async function publish(id: string, body: unknown) {
    const { POST } = await import("./[id]/versions/route.js");
    return POST(
      new Request(`http://localhost/api/workflows/${id}/versions`, {
        method: "POST",
        body: JSON.stringify(body),
      }),
      { params: Promise.resolve({ id }) },
    );
  }

  it("creates a workflow at version 1 and audits it", async () => {
    const res = await post({ name: "Weekly order", steps });
    expect(res.status).toBe(201);
    const { workflowId, version } = (await res.json()) as { workflowId: string; version: number };
    expect(version).toBe(1);

    const stored = await prisma.workflowVersion.findFirstOrThrow({ where: { workflowId } });
    expect(stored.version).toBe(1);
    expect(stored.steps).toHaveLength(3);

    const audit = await prisma.auditEvent.findFirst({
      where: { orgId: orgA, entityId: workflowId, action: "workflow.version_published" },
    });
    expect(audit?.metadata).toMatchObject({ version: 1, stepCount: 3 });
  });

  it("publishes monotonically increasing versions without mutating the old one", async () => {
    const created = await post({ name: "Versioned", steps });
    const { workflowId } = (await created.json()) as { workflowId: string };

    const second = await publish(workflowId, { steps: steps.slice(0, 2), note: "trim" });
    expect(second.status).toBe(201);
    expect((await second.json()).version).toBe(2);

    const all = await prisma.workflowVersion.findMany({
      where: { workflowId },
      orderBy: { version: "asc" },
    });
    expect(all.map((v) => v.version)).toEqual([1, 2]);
    // v1 is untouched — a run already executing it keeps the steps it started with.
    expect(all[0]!.steps).toHaveLength(3);
    expect(all[1]!.steps).toHaveLength(2);
  });

  it("rejects steps the executor cannot perform", async () => {
    // `apiCall` parses and the classifier gates it, but applyStep does nothing
    // with it — a run would skip the step and report success.
    const res = await post({
      name: "Connector",
      steps: [{ id: "a", type: "apiCall", connectorId: "gmail", operation: "send" }],
    });
    expect(res.status).toBe(400);
    expect((await res.json()).error).toMatch(/no executor/i);
  });

  it("rejects malformed steps", async () => {
    const res = await post({ name: "Bad", steps: [{ id: "n", type: "navigate", url: "not-a-url" }] });
    expect(res.status).toBe(400);
  });

  it("rejects an empty workflow", async () => {
    const res = await post({ name: "Empty", steps: [] });
    expect(res.status).toBe(400);
  });

  it("will not publish into another org's workflow", async () => {
    const created = await post({ name: "Tenant A", steps });
    const { workflowId } = (await created.json()) as { workflowId: string };

    // Same workflow id, different caller. Must be indistinguishable from a
    // workflow that does not exist — not a 403 that confirms it does.
    session.current = { user: { id: userA, orgId: orgB } };
    const res = await publish(workflowId, { steps });
    expect(res.status).toBe(404);
    session.current = { user: { id: userA, orgId: orgA } };

    const versions = await prisma.workflowVersion.count({ where: { workflowId } });
    expect(versions).toBe(1);
  });

  it("refuses unauthenticated callers", async () => {
    session.current = null;
    const res = await post({ name: "Nope", steps });
    expect(res.status).toBe(401);
    session.current = { user: { id: userA, orgId: orgA } };
  });
});
