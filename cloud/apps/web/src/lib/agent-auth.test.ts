import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The agent surface is excluded from the middleware matcher (bearer clients
 * have no session), so it must refuse a browser session cookie entirely —
 * not merely MFA-gate it. Otherwise a stolen session can `POST /api/agent/runs`
 * while `/api/runs` is correctly challenged.
 *
 * These tests pin the refusal. They are hermetic: no database.
 */

vi.mock("@/lib/db", () => ({
  prisma: {
    agentCredential: {
      findUnique: async () => null,
      update: async () => undefined,
    },
    membership: { findUnique: async () => null },
  },
}));

const { resolveAgentPrincipal } = await import("@/lib/agent-auth");

function bareRequest() {
  return new Request("https://ghost.test/api/agent/runs", { method: "POST" });
}

function bearerRequest(token: string) {
  return new Request("https://ghost.test/api/agent/runs", {
    method: "POST",
    headers: { Authorization: `Bearer ${token}` },
  });
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("resolveAgentPrincipal — bearer only", () => {
  it("refuses a caller with no Authorization header", async () => {
    const result = await resolveAgentPrincipal(bareRequest());
    expect(result).toMatchObject({
      ok: false,
      status: 401,
      error: expect.stringMatching(/agent api key required/i),
    });
  });

  it("refuses an invalid bearer token", async () => {
    const result = await resolveAgentPrincipal(bearerRequest("not-a-real-key"));
    expect(result).toMatchObject({ ok: false, status: 401, error: "invalid agent api key" });
  });
});
