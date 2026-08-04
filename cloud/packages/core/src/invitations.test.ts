import { describe, expect, it } from "vitest";
import {
  checkInvitationRedeemable,
  createInvitationToken,
  emailsMatch,
  hashInvitationToken,
  invitationExpiry,
  wouldOrphanOrganization,
} from "./invitations.js";

const future = new Date(Date.now() + 60_000);
const past = new Date(Date.now() - 60_000);

function invite(over: Partial<Parameters<typeof checkInvitationRedeemable>[0]> = {}) {
  return {
    email: "invitee@example.com",
    expiresAt: future,
    acceptedAt: null,
    revokedAt: null,
    ...over,
  };
}

describe("invitation tokens", () => {
  it("never returns the same token twice and stores only a digest", () => {
    const a = createInvitationToken();
    const b = createInvitationToken();
    expect(a.token).not.toEqual(b.token);
    expect(a.tokenHash).toEqual(hashInvitationToken(a.token));
    expect(a.tokenHash).not.toContain(a.token);
  });

  it("expires by default", () => {
    const now = new Date("2026-01-01T00:00:00Z");
    expect(invitationExpiry(now, 7).toISOString()).toBe("2026-01-08T00:00:00.000Z");
  });
});

describe("checkInvitationRedeemable", () => {
  it("accepts a live invitation for the matching user", () => {
    expect(checkInvitationRedeemable(invite(), "invitee@example.com")).toEqual({ ok: true });
  });

  it("ignores case and surrounding whitespace on the email", () => {
    expect(checkInvitationRedeemable(invite(), "  INVITEE@Example.COM ")).toEqual({ ok: true });
  });

  it("refuses a token held by anyone other than the invited address", () => {
    // The point of binding to email: a forwarded or leaked link is not enough.
    expect(checkInvitationRedeemable(invite(), "someone-else@example.com")).toEqual({
      ok: false,
      reason: "email_mismatch",
    });
  });

  it("refuses a signed-in user with no email at all", () => {
    expect(checkInvitationRedeemable(invite(), null)).toEqual({
      ok: false,
      reason: "email_mismatch",
    });
  });

  it("refuses expired, revoked, already-accepted, and unknown invitations", () => {
    expect(checkInvitationRedeemable(invite({ expiresAt: past }), "invitee@example.com").ok).toBe(
      false,
    );
    expect(checkInvitationRedeemable(invite({ revokedAt: past }), "invitee@example.com")).toEqual({
      ok: false,
      reason: "revoked",
    });
    expect(checkInvitationRedeemable(invite({ acceptedAt: past }), "invitee@example.com")).toEqual({
      ok: false,
      reason: "already_accepted",
    });
    expect(checkInvitationRedeemable(null, "invitee@example.com")).toEqual({
      ok: false,
      reason: "not_found",
    });
  });

  it("treats expiry as exclusive at the boundary", () => {
    const now = new Date("2026-01-01T00:00:00Z");
    expect(checkInvitationRedeemable(invite({ expiresAt: now }), "invitee@example.com", now)).toEqual(
      { ok: false, reason: "expired" },
    );
  });
});

describe("emailsMatch", () => {
  it("does not match different-length addresses", () => {
    expect(emailsMatch("a@example.com", "ab@example.com")).toBe(false);
  });
});

describe("wouldOrphanOrganization", () => {
  it("blocks removing the only owner", () => {
    expect(
      wouldOrphanOrganization({
        currentRoles: ["OWNER", "MEMBER"],
        targetCurrentRole: "OWNER",
        targetNextRole: null,
      }),
    ).toBe(true);
  });

  it("blocks demoting the only owner — an org with no owner cannot be administered", () => {
    expect(
      wouldOrphanOrganization({
        currentRoles: ["OWNER", "ADMIN"],
        targetCurrentRole: "OWNER",
        targetNextRole: "ADMIN",
      }),
    ).toBe(true);
  });

  it("allows removing an owner when another remains", () => {
    expect(
      wouldOrphanOrganization({
        currentRoles: ["OWNER", "OWNER"],
        targetCurrentRole: "OWNER",
        targetNextRole: null,
      }),
    ).toBe(false);
  });

  it("does not restrict non-owners", () => {
    expect(
      wouldOrphanOrganization({
        currentRoles: ["OWNER", "ADMIN"],
        targetCurrentRole: "ADMIN",
        targetNextRole: null,
      }),
    ).toBe(false);
  });
});
