import type { Role } from "@prisma/client";

/**
 * Capability checks over `Membership.role`.
 *
 * `Role` has been stored since Phase 0; until recently almost nothing read it.
 * Every existing single-member org is `OWNER`, so gating publish/approve to
 * admins changes nothing for those orgs — it only matters once an org has
 * MEMBERs who must not expand or authorize mutating work.
 *
 * VIEWER / APPROVER are deliberately not in the enum yet. Until they are, the
 * three-role matrix is:
 *
 *   | Capability        | OWNER/ADMIN | MEMBER |
 *   | publish / create  | yes         | no     |
 *   | approve (not reject) | yes      | no     |
 *   | start a run       | yes         | yes    |
 *   | reject a gate     | yes         | yes    |
 *
 * Reject stays open to every member: stopping an action is not authorizing it,
 * and whoever started a runaway run has to be able to halt it.
 */

const ADMIN_ROLES: ReadonlySet<Role> = new Set(["OWNER", "ADMIN"]);

export function isOrgAdmin(role: Role): boolean {
  return ADMIN_ROLES.has(role);
}

/** Create a workflow or publish a new version of one. */
export function canPublishWorkflow(role: Role): boolean {
  return isOrgAdmin(role);
}

/**
 * Approve a pending sensitive-step gate. Reject is deliberately not gated —
 * see the module comment.
 */
export function canApproveRun(role: Role): boolean {
  return isOrgAdmin(role);
}

/** Enqueue a run of a published workflow. Any member may trigger. */
export function canStartRun(_role: Role): boolean {
  return true;
}
