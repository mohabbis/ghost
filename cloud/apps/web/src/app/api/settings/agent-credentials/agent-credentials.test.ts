import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { createAgentToken } from "@ghost/core/agent-credentials";
import { prisma } from "@ghost/core/db";

/**
 * Agent credential hardening: optional expiry at creation, org-admin
 * inventory/revocation, and admin-only minting.
 *
 * Revocation and expiry *enforcement* already existed (`resolveAgentPrincipal`
 * rejects a revoked or expired credential). Minting is now OWNER/ADMIN only —
 * a MEMBER must not expand the agent attack surface by creating keys that can
 * start runs.
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

const session = vi.hoisted(() => ({
  current: null as null | { user: { id: string; orgId: string } },
}));
vi.mock("@/auth", () => ({ auth: async () => session.current }));

describe.skipIf(!hasDb)("agent credential hardening (Postgres)", () => {
  let orgId: string;
  let owner: string;
  let member: string;
  const slug = `agent-cred-${Date.now()}`;

  beforeAll(async () => {
    const org = await prisma.organization.create({ data: { name: "Agent Creds", slug } });
    orgId = org.id;
    const a = await prisma.user.create({
      data: { email: `${slug}-owner@example.com`, memberships: { create: { orgId, role: "OWNER" } } },
    });
    const b = await prisma.user.create({
      data: { email: `${slug}-member@example.com`, memberships: { create: { orgId, role: "MEMBER" } } },
    });
    owner = a.id;
    member = b.id;
  });

  afterAll(async () => {
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.user.deleteMany({ where: { id: { in: [owner, member] } } });
  });

  beforeEach(() => {
    // Minting is admin-only; most create tests run as OWNER.
    session.current = { user: { id: owner, orgId } };
  });

  async function create(body: Record<string, unknown>) {
    const { POST } = await import("./route.js");
    return POST(
      new Request("http://localhost/api/settings/agent-credentials", {
        method: "POST",
        body: JSON.stringify(body),
      }),
    );
  }

  async function revoke(id: string) {
    const { DELETE } = await import("./[id]/route.js");
    return DELETE(
      new Request(`http://localhost/api/settings/agent-credentials/${id}`, { method: "DELETE" }),
      { params: Promise.resolve({ id }) },
    );
  }

  async function orgList() {
    const { GET } = await import("./org/route.js");
    return GET();
  }

  /** Seed a credential owned by `userId` without going through the HTTP mint gate. */
  async function seedCredential(userId: string, name: string) {
    const generated = createAgentToken();
    return prisma.agentCredential.create({
      data: {
        orgId,
        userId,
        name,
        tokenHash: generated.tokenHash,
        tokenHint: generated.tokenHint,
      },
      select: { id: true },
    });
  }

  it("creates a credential with no expiry by default", async () => {
    const res = await create({ name: "no-expiry" });
    expect(res.status).toBe(201);
    const body = (await res.json()) as { credential: { id: string; expiresAt: string | null } };
    expect(body.credential.expiresAt).toBeNull();
  });

  it("sets expiresAt when expiresInDays is given", async () => {
    const before = Date.now();
    const res = await create({ name: "30-day", expiresInDays: 30 });
    expect(res.status).toBe(201);
    const body = (await res.json()) as { credential: { expiresAt: string } };
    const expiresAt = new Date(body.credential.expiresAt).getTime();
    expect(expiresAt).toBeGreaterThan(before + 29 * 86_400_000);
    expect(expiresAt).toBeLessThan(before + 31 * 86_400_000);
  });

  it.each([0, -1, 1.5, 3651, "not-a-number"])(
    "rejects an invalid expiresInDays (%s)",
    async (value) => {
      const res = await create({ name: "bad-expiry", expiresInDays: value });
      expect(res.status).toBe(400);
      const created = await prisma.agentCredential.findFirst({ where: { orgId, name: "bad-expiry" } });
      expect(created).toBeNull();
    },
  );

  it("refuses minting to a non-admin MEMBER", async () => {
    session.current = { user: { id: member, orgId } };
    const res = await create({ name: "member-mint" });
    expect(res.status).toBe(403);
    const created = await prisma.agentCredential.findFirst({ where: { orgId, name: "member-mint" } });
    expect(created).toBeNull();
  });

  it("lets a member revoke their own credential but not a colleague's", async () => {
    const ownerCred = (await (await create({ name: "owner-cred" })).json() as {
      credential: { id: string };
    }).credential;

    const memberCred = await seedCredential(member, "member-cred");

    session.current = { user: { id: member, orgId } };
    const ownRevoke = await revoke(memberCred.id);
    expect(ownRevoke.status).toBe(200);

    const otherRevoke = await revoke(ownerCred.id);
    expect(otherRevoke.status).toBe(404);
    const stillActive = await prisma.agentCredential.findUniqueOrThrow({
      where: { id: ownerCred.id },
    });
    expect(stillActive.revokedAt).toBeNull();
  });

  it("lets an OWNER revoke a member's credential", async () => {
    const memberCred = await seedCredential(member, "revoke-me");

    session.current = { user: { id: owner, orgId } };
    const res = await revoke(memberCred.id);
    expect(res.status).toBe(200);
    const row = await prisma.agentCredential.findUniqueOrThrow({ where: { id: memberCred.id } });
    expect(row.revokedAt).not.toBeNull();
  });

  it("refuses the org-wide credential list to a non-admin", async () => {
    session.current = { user: { id: member, orgId } };
    const res = await orgList();
    expect(res.status).toBe(403);
  });

  it("shows an OWNER every active credential in the org, across members", async () => {
    await seedCredential(member, "member-visible-to-owner");

    session.current = { user: { id: owner, orgId } };
    const res = await orgList();
    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      credentials: Array<{ name: string; user: { email: string | null } }>;
    };
    const names = body.credentials.map((c) => c.name);
    expect(names).toContain("member-visible-to-owner");
    const entry = body.credentials.find((c) => c.name === "member-visible-to-owner");
    expect(entry?.user.email).toBe(`${slug}-member@example.com`);
  });
});
