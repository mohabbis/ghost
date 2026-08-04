import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { prisma } from "@ghost/core/db";
import { createInvitationToken, invitationExpiry } from "@ghost/core/invitations";

/**
 * Membership is the boundary that decides who may act inside a tenant, so the
 * rules that keep it sane get integration tests against a real database:
 *
 *   - a leaked invite link cannot be redeemed by the wrong person;
 *   - an organization cannot be left with no owner;
 *   - non-admins cannot invite, promote, or remove;
 *   - one invitation cannot be redeemed twice.
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

const session = vi.hoisted(() => ({
  current: null as null | { user: { id: string; orgId: string; email?: string } },
}));
vi.mock("@/auth", () => ({ auth: async () => session.current }));

const { POST: invite } = await import("@/app/api/settings/members/invitations/route");
const { POST: acceptInvite } = await import("@/app/api/invitations/accept/route");
const { PATCH: patchMember, DELETE: removeMember } = await import(
  "@/app/api/settings/members/[userId]/route"
);

function jsonReq(body: unknown) {
  return new Request("http://localhost/x", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}
const params = (userId: string) => ({ params: Promise.resolve({ userId }) });

describe.skipIf(!hasDb)("members & invitations (Postgres)", () => {
  const slug = `members-${Date.now()}`;
  let orgId: string;
  let ownerId: string;
  let memberId: string;
  let outsiderId: string;

  beforeAll(async () => {
    const org = await prisma.organization.create({ data: { name: "Members", slug } });
    orgId = org.id;

    const owner = await prisma.user.create({
      data: {
        email: `owner-${slug}@example.com`,
        memberships: { create: { orgId, role: "OWNER" } },
      },
    });
    ownerId = owner.id;

    const member = await prisma.user.create({
      data: {
        email: `member-${slug}@example.com`,
        memberships: { create: { orgId, role: "MEMBER" } },
      },
    });
    memberId = member.id;

    const outsider = await prisma.user.create({ data: { email: `outsider-${slug}@example.com` } });
    outsiderId = outsider.id;
  });

  afterAll(async () => {
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.user
      .deleteMany({ where: { id: { in: [ownerId, memberId, outsiderId].filter(Boolean) } } })
      .catch(() => undefined);
  });

  beforeEach(() => {
    session.current = { user: { id: ownerId, orgId } };
  });

  async function seedInvitation(email: string, role: "OWNER" | "ADMIN" | "MEMBER" = "MEMBER") {
    const { token, tokenHash } = createInvitationToken();
    await prisma.invitation.create({
      data: { orgId, email, role, tokenHash, expiresAt: invitationExpiry(), createdById: ownerId },
    });
    return token;
  }

  it("refuses to invite unless the caller is an owner or admin", async () => {
    session.current = { user: { id: memberId, orgId } };
    const res = await invite(jsonReq({ email: "nope@example.com" }));
    expect(res.status).toBe(403);
    expect(await prisma.invitation.count({ where: { orgId, email: "nope@example.com" } })).toBe(0);
  });

  it("issues an invitation whose token is never stored in plaintext", async () => {
    const res = await invite(jsonReq({ email: `New.Person-${slug}@Example.com`, role: "ADMIN" }));
    expect(res.status).toBe(201);
    const body = (await res.json()) as { acceptUrl: string };

    const token = body.acceptUrl.split("/").pop()!;
    const row = await prisma.invitation.findFirstOrThrow({
      where: { orgId, email: `new.person-${slug}@example.com` },
    });
    // Normalized on write, and only the digest is persisted.
    expect(row.email).toBe(`new.person-${slug}@example.com`);
    expect(row.role).toBe("ADMIN");
    expect(row.tokenHash).not.toContain(token);
  });

  it("refuses a leaked link redeemed by anyone other than the invited address", async () => {
    const token = await seedInvitation(`invited-${slug}@example.com`);
    // Signed in as a different person who got hold of the link.
    session.current = { user: { id: outsiderId, orgId } };

    const res = await acceptInvite(jsonReq({ token }));
    expect(res.status).toBe(404);
    expect(await prisma.membership.count({ where: { orgId, userId: outsiderId } })).toBe(0);
    // Still unredeemed — a failed attempt must not burn the invitation.
    const row = await prisma.invitation.findFirstOrThrow({
      where: { orgId, email: `invited-${slug}@example.com` },
    });
    expect(row.acceptedAt).toBeNull();
  });

  it("lets the invited address join, exactly once", async () => {
    const email = `joiner-${slug}@example.com`;
    const joiner = await prisma.user.create({ data: { email } });
    const token = await seedInvitation(email, "MEMBER");
    session.current = { user: { id: joiner.id, orgId } };

    const first = await acceptInvite(jsonReq({ token }));
    expect(first.status).toBe(200);
    expect(await prisma.membership.count({ where: { orgId, userId: joiner.id } })).toBe(1);

    // The same person revisiting their own spent link is told they are
    // already in, not that the invitation is invalid — the normal path when
    // `ensureUserOrg` consumed it during sign-in. Still exactly one
    // membership: the token is spent, only the message differs.
    const second = await acceptInvite(jsonReq({ token }));
    expect(second.status).toBe(200);
    expect(await second.json()).toMatchObject({ alreadyMember: true });
    expect(await prisma.membership.count({ where: { orgId, userId: joiner.id } })).toBe(1);

    // Anyone else holding that spent token still learns nothing.
    session.current = { user: { id: outsiderId, orgId } };
    expect((await acceptInvite(jsonReq({ token }))).status).toBe(404);
    expect(await prisma.membership.count({ where: { orgId, userId: outsiderId } })).toBe(0);

    await prisma.user.delete({ where: { id: joiner.id } }).catch(() => undefined);
  });

  it("refuses an expired invitation", async () => {
    const email = `stale-${slug}@example.com`;
    const stale = await prisma.user.create({ data: { email } });
    const { token, tokenHash } = createInvitationToken();
    await prisma.invitation.create({
      data: {
        orgId,
        email,
        role: "MEMBER",
        tokenHash,
        expiresAt: new Date(Date.now() - 1000),
        createdById: ownerId,
      },
    });
    session.current = { user: { id: stale.id, orgId } };

    expect((await acceptInvite(jsonReq({ token }))).status).toBe(404);
    expect(await prisma.membership.count({ where: { orgId, userId: stale.id } })).toBe(0);

    await prisma.user.delete({ where: { id: stale.id } }).catch(() => undefined);
  });

  it("will not demote or remove the last owner", async () => {
    const demote = await patchMember(jsonReq({ role: "MEMBER" }), params(ownerId));
    expect(demote.status).toBe(409);

    const remove = await removeMember(new Request("http://localhost/x"), params(ownerId));
    expect(remove.status).toBe(409);

    const still = await prisma.membership.findFirstOrThrow({ where: { orgId, userId: ownerId } });
    expect(still.role).toBe("OWNER");
  });

  it("allows demoting an owner once a second owner exists", async () => {
    await prisma.membership.updateMany({
      where: { orgId, userId: memberId },
      data: { role: "OWNER" },
    });

    const res = await patchMember(jsonReq({ role: "MEMBER" }), params(ownerId));
    expect(res.status).toBe(200);

    // Put it back for the remaining tests.
    await prisma.membership.updateMany({
      where: { orgId, userId: ownerId },
      data: { role: "OWNER" },
    });
    await prisma.membership.updateMany({
      where: { orgId, userId: memberId },
      data: { role: "MEMBER" },
    });
  });

  it("refuses role changes and removals from a non-admin", async () => {
    session.current = { user: { id: memberId, orgId } };
    expect((await patchMember(jsonReq({ role: "OWNER" }), params(memberId))).status).toBe(403);
    expect(
      (await removeMember(new Request("http://localhost/x"), params(ownerId))).status,
    ).toBe(403);
  });

  it("does not let an admin in one org touch another org's member", async () => {
    const otherOrg = await prisma.organization.create({
      data: { name: "Other", slug: `${slug}-other` },
    });
    session.current = { user: { id: ownerId, orgId: otherOrg.id } };

    // Owner of `orgId`, but acting in an org they do not belong to at all.
    expect((await patchMember(jsonReq({ role: "MEMBER" }), params(memberId))).status).toBe(403);

    await prisma.organization.delete({ where: { id: otherOrg.id } }).catch(() => undefined);
  });
});
