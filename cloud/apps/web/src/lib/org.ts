import { appendAuditEvent } from "@ghost/core/audit-log";
import { normalizeEmail } from "@ghost/core/invitations";
import { prisma } from "./db";

/**
 * Consume the newest live invitation for `email`, if there is one, and return
 * the organization joined.
 *
 * Only ever called for a user with no memberships at all, so there is no
 * question of it moving an existing user between tenants.
 */
async function claimPendingInvitation(userId: string, email: string): Promise<string | null> {
  const normalized = normalizeEmail(email);

  return prisma.$transaction(async (tx) => {
    const invitation = await tx.invitation.findFirst({
      where: {
        email: normalized,
        acceptedAt: null,
        revokedAt: null,
        expiresAt: { gt: new Date() },
      },
      orderBy: { createdAt: "desc" },
      select: { id: true, orgId: true, role: true },
    });
    if (!invitation) return null;

    // Conditional claim, so two concurrent sign-ins cannot both redeem it.
    const claimed = await tx.invitation.updateMany({
      where: { id: invitation.id, acceptedAt: null, revokedAt: null },
      data: { acceptedAt: new Date(), acceptedById: userId },
    });
    if (claimed.count === 0) return null;

    await tx.membership.create({
      data: { orgId: invitation.orgId, userId, role: invitation.role },
    });
    await appendAuditEvent(
      invitation.orgId,
      userId,
      {
        action: "member.joined",
        entityType: "Membership",
        entityId: userId,
        metadata: { role: invitation.role, invitationId: invitation.id, via: "sign_in" },
      },
      tx,
    );
    return invitation.orgId;
  });
}

/** Turn an email/name into a readable org name + unique-ish slug. */
function deriveOrg(email: string, name?: string | null): { name: string; slug: string } {
  const handle = (name?.trim() || email.split("@")[0] || "workspace").trim();
  const base =
    handle
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "workspace";
  const suffix = Math.random().toString(36).slice(2, 7);
  return { name: `${handle}'s workspace`, slug: `${base}-${suffix}` };
}

/**
 * Ensure a signed-in user has a persisted `User`, an `Organization`, and a
 * `Membership`. Idempotent — safe to call on every sign-in. Returns the ids the
 * session needs. This is what makes the app multi-tenant from the first login.
 */
export async function ensureUserOrg(
  email: string,
  name?: string | null,
  image?: string | null,
): Promise<{ userId: string; orgId: string }> {
  const user = await prisma.user.upsert({
    where: { email },
    update: { name: name ?? undefined, image: image ?? undefined },
    create: { email, name: name ?? undefined, image: image ?? undefined },
  });

  const existing = await prisma.membership.findFirst({
    where: { userId: user.id },
    orderBy: { createdAt: "asc" },
  });
  if (existing) return { userId: user.id, orgId: existing.orgId };

  // Someone invited here who has never used Ghost joins the inviting
  // organization instead of getting a personal one. Without this they would
  // sign in, land in a fresh empty workspace, and have to find the invite link
  // again — and because session `orgId` is stamped at sign-in, accepting would
  // appear to do nothing until they signed in a second time.
  //
  // The invitation is consumed here rather than merely read: this *is* the
  // acceptance. The same email/expiry/revocation rules as the accept route
  // apply, minus the email check, which is trivially satisfied — we are
  // looking up by the address the provider just authenticated.
  const invited = await claimPendingInvitation(user.id, email);
  if (invited) return { userId: user.id, orgId: invited };

  const { name: orgName, slug } = deriveOrg(email, name);
  const org = await prisma.organization.create({
    data: {
      name: orgName,
      slug,
      memberships: { create: { userId: user.id, role: "OWNER" } },
    },
  });
  return { userId: user.id, orgId: org.id };
}
