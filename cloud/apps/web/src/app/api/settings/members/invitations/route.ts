import { appendAuditEvent } from "@ghost/core/audit-log";
import {
  createInvitationToken,
  invitationExpiry,
  normalizeEmail,
} from "@ghost/core/invitations";
import { auth } from "@/auth";
import { appUrl } from "@/lib/app-url";
import { prisma } from "@/lib/db";
import { actorIsAdmin, isAssignableRole, loadActor } from "@/lib/members";

/**
 * Invite someone to this organization.
 *
 * Admin-only. Returns the accept link exactly once — only the token's digest
 * is stored, so it cannot be shown again, the same contract as an agent
 * credential. There is no mail delivery yet: the inviter copies the link and
 * sends it. That is a real limitation, not an oversight — see the response's
 * `deliver` field, which says so rather than letting the UI imply an email
 * went out.
 */
export async function POST(req: Request): Promise<Response> {
  const session = await auth();
  if (!session?.user?.orgId || !session.user.id) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const actor = await loadActor(orgId, session.user.id);
  if (!actorIsAdmin(actor)) {
    return Response.json({ error: "only an owner or admin may invite" }, { status: 403 });
  }

  const body = (await req.json().catch(() => null)) as
    | { email?: unknown; role?: unknown }
    | null;
  const rawEmail = typeof body?.email === "string" ? body.email : "";
  const email = normalizeEmail(rawEmail);
  if (!email || !email.includes("@")) {
    return Response.json({ error: "a valid email is required" }, { status: 400 });
  }
  const role = body?.role === undefined ? "MEMBER" : body.role;
  if (!isAssignableRole(role)) {
    return Response.json({ error: "unknown role" }, { status: 400 });
  }

  // Already in this org: inviting again would create a token that can never be
  // redeemed (acceptance is idempotent for existing members), so say so.
  const existing = await prisma.membership.findFirst({
    where: { orgId, user: { email } },
    select: { id: true },
  });
  if (existing) {
    return Response.json({ error: "that person is already a member" }, { status: 409 });
  }

  const { token, tokenHash } = createInvitationToken();

  const invitation = await prisma.$transaction(async (tx) => {
    // Supersede any outstanding invitation for the same address, so a
    // re-invite does not leave two live tokens for one seat.
    await tx.invitation.updateMany({
      where: { orgId, email, acceptedAt: null, revokedAt: null },
      data: { revokedAt: new Date() },
    });

    const created = await tx.invitation.create({
      data: {
        orgId,
        email,
        role,
        tokenHash,
        expiresAt: invitationExpiry(),
        createdById: actor.userId,
      },
      select: { id: true, email: true, role: true, expiresAt: true },
    });

    await appendAuditEvent(
      orgId,
      actor.userId,
      {
        action: "member.invited",
        entityType: "Invitation",
        entityId: created.id,
        // The address is the point of the record; the token never appears.
        metadata: { email, role },
      },
      tx,
    );

    return created;
  });

  return Response.json(
    {
      invitation,
      acceptUrl: `${appUrl()}/invitations/${token}`,
      deliver:
        "Ghost does not send this email yet — copy the link and send it yourself. It is shown once.",
    },
    { status: 201 },
  );
}

/** Outstanding invitations for this organization. Admin-only. */
export async function GET(): Promise<Response> {
  const session = await auth();
  if (!session?.user?.orgId || !session.user.id) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const actor = await loadActor(session.user.orgId, session.user.id);
  if (!actorIsAdmin(actor)) {
    return Response.json({ error: "only an owner or admin may view invitations" }, { status: 403 });
  }

  const invitations = await prisma.invitation.findMany({
    where: { orgId: session.user.orgId, acceptedAt: null, revokedAt: null },
    orderBy: { createdAt: "desc" },
    // No tokenHash: it is useless to the client and pointless to expose.
    select: { id: true, email: true, role: true, expiresAt: true, createdAt: true },
  });

  return Response.json({ invitations });
}
