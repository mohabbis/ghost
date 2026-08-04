import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import type { Role } from "@prisma/client";

/**
 * Organization invitations.
 *
 * Same shape as agent credentials: a high-entropy token is generated once,
 * only its SHA-256 digest is stored, and the plaintext is unrecoverable
 * afterwards. Two things are deliberately different, because an invitation
 * grants standing access to a tenant rather than a scoped API capability:
 *
 *   1. It expires. An invite left in an inbox for a year is a way into the
 *      organization long after whoever sent it stopped meaning it.
 *   2. Holding the token is not sufficient. Acceptance also requires the
 *      signed-in user's email to match the invited address, so a forwarded
 *      or leaked link cannot be redeemed by whoever happens to find it.
 */

const TOKEN_PREFIX = "ghost_invite_";

export const DEFAULT_INVITATION_TTL_DAYS = 7;

export function hashInvitationToken(token: string): string {
  return createHash("sha256").update(token, "utf8").digest("hex");
}

export function createInvitationToken(): { token: string; tokenHash: string } {
  const token = `${TOKEN_PREFIX}${randomBytes(32).toString("base64url")}`;
  return { token, tokenHash: hashInvitationToken(token) };
}

export function invitationExpiry(
  now: Date = new Date(),
  ttlDays: number = DEFAULT_INVITATION_TTL_DAYS,
): Date {
  return new Date(now.getTime() + ttlDays * 24 * 60 * 60 * 1000);
}

/** Emails are compared, never displayed-then-trusted, so normalize on both sides. */
export function normalizeEmail(email: string): string {
  return email.trim().toLowerCase();
}

/**
 * Constant-time comparison of two already-normalized emails.
 *
 * Not a meaningful timing oracle on its own — the attacker supplies the token,
 * not the email — but the comparison is free to do properly and it keeps the
 * "compare secrets in constant time" habit intact where someone later copies
 * this code somewhere it does matter.
 */
export function emailsMatch(a: string, b: string): boolean {
  const left = Buffer.from(normalizeEmail(a), "utf8");
  const right = Buffer.from(normalizeEmail(b), "utf8");
  if (left.length !== right.length) return false;
  return timingSafeEqual(left, right);
}

export type InvitationRejection =
  | "not_found"
  | "revoked"
  | "already_accepted"
  | "expired"
  | "email_mismatch";

export interface InvitationLike {
  email: string;
  expiresAt: Date;
  acceptedAt: Date | null;
  revokedAt: Date | null;
}

/**
 * Whether an invitation may be redeemed by this user, right now.
 *
 * Pure, so every rule is testable without a database. Returns the specific
 * reason rather than a boolean: the caller decides how much to disclose (the
 * API deliberately collapses these to one message, so probing a token cannot
 * reveal whether it merely expired versus never existed).
 */
export function checkInvitationRedeemable(
  invitation: InvitationLike | null,
  userEmail: string | null | undefined,
  now: Date = new Date(),
): { ok: true } | { ok: false; reason: InvitationRejection } {
  if (!invitation) return { ok: false, reason: "not_found" };
  if (invitation.revokedAt) return { ok: false, reason: "revoked" };
  if (invitation.acceptedAt) return { ok: false, reason: "already_accepted" };
  if (invitation.expiresAt.getTime() <= now.getTime()) {
    return { ok: false, reason: "expired" };
  }
  if (!userEmail || !emailsMatch(invitation.email, userEmail)) {
    return { ok: false, reason: "email_mismatch" };
  }
  return { ok: true };
}

/**
 * Whether a membership change would leave the organization with no OWNER.
 *
 * An org with no owner cannot invite, cannot change roles, and cannot revoke
 * anything — it is administratively dead, and no ordinary member can revive
 * it. Both removal and demotion have to be checked; demoting the last owner
 * to MEMBER bricks the org just as thoroughly as deleting them.
 */
export function wouldOrphanOrganization(input: {
  /** Current roles of every member, including the one being changed. */
  currentRoles: readonly Role[];
  /** Role of the member being changed. */
  targetCurrentRole: Role;
  /** Their role after the change, or null when they are being removed. */
  targetNextRole: Role | null;
}): boolean {
  const { currentRoles, targetCurrentRole, targetNextRole } = input;
  if (targetCurrentRole !== "OWNER") return false;
  if (targetNextRole === "OWNER") return false;
  const owners = currentRoles.filter((r) => r === "OWNER").length;
  return owners <= 1;
}
