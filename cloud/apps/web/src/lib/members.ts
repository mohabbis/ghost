import type { Role } from "@ghost/core/db";
import { isOrgAdmin } from "@ghost/core/roles";
import { prisma } from "@/lib/db";

/**
 * Shared authorization for the member/invitation routes.
 *
 * Every one of these routes changes who can act inside a tenant, so the role
 * check is re-read from the database on each request rather than trusted from
 * the session token: the JWT is minted at sign-in and says nothing about a
 * demotion that happened since.
 */

export interface OrgActor {
  userId: string;
  orgId: string;
  role: Role;
}

export async function loadActor(orgId: string, userId: string): Promise<OrgActor | null> {
  const membership = await prisma.membership.findFirst({
    where: { orgId, userId },
    select: { role: true },
  });
  if (!membership) return null;
  return { userId, orgId, role: membership.role };
}

export function actorIsAdmin(actor: OrgActor | null): actor is OrgActor {
  return actor !== null && isOrgAdmin(actor.role);
}

export const ASSIGNABLE_ROLES: readonly Role[] = ["OWNER", "ADMIN", "MEMBER"];

export function isAssignableRole(value: unknown): value is Role {
  return typeof value === "string" && (ASSIGNABLE_ROLES as readonly string[]).includes(value);
}
