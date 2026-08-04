import { appendAuditEvent } from "@ghost/core/audit-log";
import {
  checkInvitationRedeemable,
  emailsMatch,
  hashInvitationToken,
} from "@ghost/core/invitations";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { clientKey, rateLimit, tooManyRequests } from "@/lib/rate-limit";

/**
 * Redeem an invitation token as the signed-in user.
 *
 * Two properties matter here:
 *
 *   1. **Holding the token is not enough.** The signed-in user's email must
 *      match the invited address, so a forwarded link cannot be redeemed by
 *      whoever finds it. `checkInvitationRedeemable` owns that rule.
 *   2. **Every rejection looks identical.** Expired, revoked, already used,
 *      wrong recipient, and never existed all return the same 404 and the
 *      same message. Distinguishing them would let anyone holding a URL probe
 *      whether an organization ever invited a given address.
 *
 * The lookup is by digest, so a token is never compared in the database as
 * plaintext and never logged.
 */
export async function POST(req: Request): Promise<Response> {
  const session = await auth();
  if (!session?.user?.id) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const userId = session.user.id;

  // An invitation token is 32 random bytes, so guessing one is not a practical
  // attack. Throttled anyway because this endpoint is a free, unauthenticated-
  // in-effect oracle otherwise: it is the one place a caller can submit
  // candidate tokens as fast as the database will answer, and that is worth
  // bounding on its own terms.
  const perUser = await rateLimit(`invite:user:${userId}`, { limit: 20, windowSeconds: 300 });
  if (!perUser.ok) return tooManyRequests(perUser);
  const perClient = await rateLimit(`invite:ip:${clientKey(req)}`, {
    limit: 60,
    windowSeconds: 300,
  });
  if (!perClient.ok) return tooManyRequests(perClient);

  const body = (await req.json().catch(() => null)) as { token?: unknown } | null;
  const token = typeof body?.token === "string" ? body.token.trim() : "";
  if (!token) return Response.json({ error: "token required" }, { status: 400 });

  // The session's email can be stale; the User row is the authority on who
  // this actually is.
  const user = await prisma.user.findUnique({
    where: { id: userId },
    select: { email: true },
  });

  const invitation = await prisma.invitation.findUnique({
    where: { tokenHash: hashInvitationToken(token) },
    select: {
      id: true,
      orgId: true,
      email: true,
      role: true,
      expiresAt: true,
      acceptedAt: true,
      revokedAt: true,
      org: { select: { name: true } },
    },
  });

  const verdict = checkInvitationRedeemable(invitation, user?.email);
  if (!verdict.ok) {
    // One exception to the undifferentiated rejection: this user already
    // redeemed this invitation and is a member of that organization.
    //
    // It is the normal path for someone invited who had never used Ghost —
    // `ensureUserOrg` consumes the invitation during sign-in so they land in
    // the right workspace, and they then arrive here holding a link that is,
    // correctly, already spent. Reporting "not valid for this account" to
    // someone who just successfully joined is simply wrong, and it is what
    // this flow did until it was tested end to end.
    //
    // It discloses nothing: the caller is provably already inside the
    // organization, so confirming their own membership tells them nothing
    // they cannot read off the members list.
    if (
      invitation &&
      verdict.reason === "already_accepted" &&
      user?.email &&
      emailsMatch(invitation.email, user.email)
    ) {
      const membership = await prisma.membership.findFirst({
        where: { orgId: invitation.orgId, userId },
        select: { role: true },
      });
      if (membership) {
        return Response.json({
          orgId: invitation.orgId,
          orgName: invitation.org.name,
          role: membership.role,
          alreadyMember: true,
          reauthRequired: false,
        });
      }
    }

    // Deliberately one undifferentiated response — see the note above.
    return Response.json(
      { error: "that invitation is not valid for this account" },
      { status: 404 },
    );
  }
  // `checkInvitationRedeemable` returning ok implies a non-null invitation;
  // narrow for the type system.
  if (!invitation) {
    return Response.json({ error: "that invitation is not valid for this account" }, { status: 404 });
  }

  const result = await prisma.$transaction(async (tx) => {
    // Re-read inside the transaction and only consume an invitation that is
    // still unaccepted, so two clicks on the same link cannot both redeem it.
    const claimed = await tx.invitation.updateMany({
      where: { id: invitation.id, acceptedAt: null, revokedAt: null },
      data: { acceptedAt: new Date(), acceptedById: userId },
    });
    if (claimed.count === 0) return { alreadyMember: false, claimed: false };

    const existing = await tx.membership.findFirst({
      where: { orgId: invitation.orgId, userId },
      select: { id: true },
    });
    if (existing) {
      // Redeeming while already a member is a no-op on access rather than a
      // duplicate membership or an unexplained error.
      return { alreadyMember: true, claimed: true };
    }

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
        metadata: { role: invitation.role, invitationId: invitation.id },
      },
      tx,
    );

    return { alreadyMember: false, claimed: true };
  });

  if (!result.claimed) {
    return Response.json(
      { error: "that invitation is not valid for this account" },
      { status: 404 },
    );
  }

  return Response.json({
    orgId: invitation.orgId,
    orgName: invitation.org.name,
    role: invitation.role,
    alreadyMember: result.alreadyMember,
    // Session `orgId` is stamped into the JWT at sign-in and there is no org
    // switcher yet, so a user who already belonged to another org keeps
    // landing there until they sign in again. Said plainly rather than
    // leaving them wondering why nothing changed.
    reauthRequired: true,
  });
}
