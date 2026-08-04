import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { prisma } from "./db.js";
import { appendAuditEvent, OrgAuditChainIntegrityError } from "./auditLog.js";

/**
 * Org audit chain head tracking.
 *
 * `verifyAuditChain` (audit.test.ts) is a pure function over whatever array of
 * events it's handed, so it cannot by itself detect a deleted suffix — the
 * surviving prefix still verifies as intact, just shorter. This is the same
 * gap Run.journalHead closed for run journals: an expected tail persisted
 * outside the chain itself, checked on every append.
 *
 * Organization.auditChainHead is that column for the org-wide chain.
 * appendAuditEvent now refuses to build on a tail that doesn't match it,
 * rather than silently accepting whatever the AuditEvent table currently
 * contains as "the chain".
 *
 * This still does not withstand an attacker able to rewrite both AuditEvent
 * and Organization.auditChainHead together — that requires an anchor outside
 * this database, which remains open. See docs/CURSOR_HANDOFF.md "Known gaps
 * in compensation (undo)".
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

describe.skipIf(!hasDb)("org audit chain head (Postgres)", () => {
  let orgId: string;
  const slug = `audit-head-${Date.now()}`;

  beforeAll(async () => {
    const org = await prisma.organization.create({ data: { name: "Audit Head", slug } });
    orgId = org.id;
  });

  afterAll(async () => {
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
  });

  it("starts null and advances with each append", async () => {
    const before = await prisma.organization.findUniqueOrThrow({ where: { id: orgId } });
    expect(before.auditChainHead).toBeNull();

    const first = await appendAuditEvent(orgId, null, {
      action: "run.started",
      entityType: "Run",
      entityId: "r1",
    });
    const afterFirst = await prisma.organization.findUniqueOrThrow({ where: { id: orgId } });
    expect(afterFirst.auditChainHead).toBe(first.hash);

    const second = await appendAuditEvent(orgId, null, {
      action: "run.succeeded",
      entityType: "Run",
      entityId: "r1",
    });
    const afterSecond = await prisma.organization.findUniqueOrThrow({ where: { id: orgId } });
    expect(afterSecond.auditChainHead).toBe(second.hash);
    expect(second.hash).not.toBe(first.hash);
  });

  it("refuses to append on top of a tail that doesn't match the persisted head", async () => {
    await appendAuditEvent(orgId, null, {
      action: "run.started",
      entityType: "Run",
      entityId: "r2",
    });

    // Simulate a deleted suffix: remove the row appendAuditEvent just wrote,
    // but leave Organization.auditChainHead pointing at its hash — exactly
    // what a suffix deletion looks like, since nothing else updates that
    // column. The next append must notice the mismatch rather than quietly
    // treating whatever remains as the real chain.
    const latest = await prisma.auditEvent.findFirstOrThrow({
      where: { orgId },
      orderBy: { seq: "desc" },
    });
    await prisma.auditEvent.delete({ where: { id: latest.id } });

    await expect(
      appendAuditEvent(orgId, null, {
        action: "run.started",
        entityType: "Run",
        entityId: "r3",
      }),
    ).rejects.toThrow(OrgAuditChainIntegrityError);
  });
});
