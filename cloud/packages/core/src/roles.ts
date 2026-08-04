import type { Role } from "@prisma/client";

/**
 * Roles that may act on behalf of the organization rather than only
 * themselves — inventory and revoke a colleague's agent credential, for
 * instance. `Role` has been stored on `Membership` since Phase 0 but nothing
 * has read it until now; every existing member is `OWNER` (orgs are
 * auto-created single-member on first sign-in, and there is no invite flow
 * yet), so this does not change behavior for any org that exists today. It
 * only matters once an org has more than one member.
 */
const ADMIN_ROLES: ReadonlySet<Role> = new Set(["OWNER", "ADMIN"]);

export function isOrgAdmin(role: Role): boolean {
  return ADMIN_ROLES.has(role);
}
