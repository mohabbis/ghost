import { appendAuditEvent } from "@ghost/core/audit-log";
import { wouldOrphanOrganization } from "@ghost/core/invitations";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { actorIsAdmin, isAssignableRole, loadActor, type OrgActor } from "@/lib/members";

/**
 * Change a member's role, or remove them.
 *
 * Both operations are guarded by `wouldOrphanOrganization`. An organization
 * with no OWNER cannot invite, cannot change roles, and cannot revoke
 * anything — it is administratively dead and no ordinary member can revive
 * it. Demoting the last owner bricks it exactly as thoroughly as deleting
 * them, so both paths check.
 */

type Params = { params: Promise<{ userId: string }> };

/** Explicitly discriminated on `ok` so both call sites narrow unambiguously. */
type Authorized =
  | { ok: false; response: Response }
  | { ok: true; orgId: string; actor: OrgActor; targetUserId: string };

async function authorize(userIdParam: string): Promise<Authorized> {
  const session = await auth();
  if (!session?.user?.orgId || !session.user.id) {
    return { ok: false, response: Response.json({ error: "unauthorized" }, { status: 401 }) };
  }
  const orgId = session.user.orgId;
  const actor = await loadActor(orgId, session.user.id);
  if (!actorIsAdmin(actor)) {
    return {
      ok: false,
      response: Response.json(
        { error: "only an owner or admin may manage members" },
        { status: 403 },
      ),
    };
  }
  return { ok: true, orgId, actor, targetUserId: userIdParam };
}

export async function PATCH(req: Request, { params }: Params): Promise<Response> {
  const { userId } = await params;
  const gate = await authorize(userId);
  if (!gate.ok) return gate.response;
  const { orgId, actor, targetUserId } = gate;

  const body = (await req.json().catch(() => null)) as { role?: unknown } | null;
  if (!isAssignableRole(body?.role)) {
    return Response.json({ error: "unknown role" }, { status: 400 });
  }
  const nextRole = body.role;

  const outcome = await prisma.$transaction(async (tx) => {
    const memberships = await tx.membership.findMany({
      where: { orgId },
      select: { userId: true, role: true },
    });
    const target = memberships.find((m) => m.userId === targetUserId);
    if (!target) return { status: 404 as const };
    if (target.role === nextRole) return { status: 200 as const, unchanged: true };

    if (
      wouldOrphanOrganization({
        currentRoles: memberships.map((m) => m.role),
        targetCurrentRole: target.role,
        targetNextRole: nextRole,
      })
    ) {
      return { status: 409 as const };
    }

    await tx.membership.updateMany({
      where: { orgId, userId: targetUserId },
      data: { role: nextRole },
    });
    await appendAuditEvent(
      orgId,
      actor.userId,
      {
        action: "member.role_changed",
        entityType: "Membership",
        entityId: targetUserId,
        metadata: { from: target.role, to: nextRole },
      },
      tx,
    );
    return { status: 200 as const, unchanged: false };
  });

  if (outcome.status === 404) return Response.json({ error: "not a member" }, { status: 404 });
  if (outcome.status === 409) {
    return Response.json(
      { error: "an organization must keep at least one owner" },
      { status: 409 },
    );
  }
  return Response.json({ ok: true, role: nextRole });
}

export async function DELETE(_req: Request, { params }: Params): Promise<Response> {
  const { userId } = await params;
  const gate = await authorize(userId);
  if (!gate.ok) return gate.response;
  const { orgId, actor, targetUserId } = gate;

  const outcome = await prisma.$transaction(async (tx) => {
    const memberships = await tx.membership.findMany({
      where: { orgId },
      select: { userId: true, role: true },
    });
    const target = memberships.find((m) => m.userId === targetUserId);
    if (!target) return { status: 404 as const };

    if (
      wouldOrphanOrganization({
        currentRoles: memberships.map((m) => m.role),
        targetCurrentRole: target.role,
        targetNextRole: null,
      })
    ) {
      return { status: 409 as const };
    }

    await tx.membership.deleteMany({ where: { orgId, userId: targetUserId } });
    await appendAuditEvent(
      orgId,
      actor.userId,
      {
        action: "member.removed",
        entityType: "Membership",
        entityId: targetUserId,
        metadata: { role: target.role, self: targetUserId === actor.userId },
      },
      tx,
    );
    return { status: 200 as const };
  });

  if (outcome.status === 404) return Response.json({ error: "not a member" }, { status: 404 });
  if (outcome.status === 409) {
    return Response.json(
      { error: "an organization must keep at least one owner" },
      { status: 409 },
    );
  }
  return Response.json({ ok: true });
}
