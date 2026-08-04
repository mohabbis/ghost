import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The agent surface is excluded from the middleware matcher, so the
 * second-factor gate that covers every other session-authenticated route has
 * to be enforced inside `resolveAgentPrincipal`.
 *
 * Without it, a stolen session cookie for an MFA-enabled user could start a
 * run through `POST /api/agent/runs` — against the customer's real systems —
 * while the identical action through `POST /api/runs` was correctly
 * challenged. That is the second factor covering everything except the
 * endpoint that moves money.
 *
 * These tests pin the three cases that matter. They are hermetic: no database,
 * no session, just the branch logic.
 */

const state = vi.hoisted(() => ({
  session: null as null | {
    user: { id?: string; orgId?: string; mfaEnabled?: boolean; sid?: string };
  },
  mfaCookie: undefined as string | undefined,
  mfaVerifies: false,
}));

vi.mock("@/auth", () => ({ auth: async () => state.session }));

vi.mock("next/headers", () => ({
  cookies: async () => ({ get: () => (state.mfaCookie ? { value: state.mfaCookie } : undefined) }),
}));

vi.mock("@/lib/mfa-cookie", () => ({
  MFA_COOKIE_NAME: "ghost_mfa",
  verifyMfaCookie: async () => state.mfaVerifies,
}));

vi.mock("@/lib/db", () => ({ prisma: {} }));

const { resolveAgentPrincipal } = await import("@/lib/agent-auth");

/** No Authorization header, so resolution falls to the session path. */
function sessionRequest() {
  return new Request("https://ghost.test/api/agent/runs", { method: "POST" });
}

beforeEach(() => {
  state.session = { user: { id: "u1", orgId: "o1", mfaEnabled: false, sid: "s1" } };
  state.mfaCookie = undefined;
  state.mfaVerifies = false;
});

describe("resolveAgentPrincipal — session path", () => {
  it("admits a user who has not enabled two-factor", async () => {
    const result = await resolveAgentPrincipal(sessionRequest());
    expect(result).toMatchObject({ ok: true, principal: { orgId: "o1", via: "session" } });
  });

  it("refuses an MFA-enabled user whose challenge has not been answered", async () => {
    state.session = { user: { id: "u1", orgId: "o1", mfaEnabled: true, sid: "s1" } };
    state.mfaVerifies = false;

    const result = await resolveAgentPrincipal(sessionRequest());

    // 403, matching what the middleware returns for an API path — a distinct
    // status from 401, because the caller *is* authenticated and needs to do
    // something specific about it.
    expect(result).toMatchObject({ ok: false, status: 403 });
  });

  it("admits an MFA-enabled user who has answered the challenge", async () => {
    state.session = { user: { id: "u1", orgId: "o1", mfaEnabled: true, sid: "s1" } };
    state.mfaCookie = "signed-cookie";
    state.mfaVerifies = true;

    const result = await resolveAgentPrincipal(sessionRequest());
    expect(result).toMatchObject({ ok: true, principal: { userId: "u1", via: "session" } });
  });

  it("still refuses an unauthenticated caller with 401", async () => {
    state.session = null;
    const result = await resolveAgentPrincipal(sessionRequest());
    expect(result).toMatchObject({ ok: false, status: 401 });
  });
});
