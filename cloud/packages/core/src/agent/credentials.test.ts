import { describe, expect, it } from "vitest";
import { createAgentToken, hashAgentToken } from "./credentials";

describe("agent credentials", () => {
  it("creates unique opaque tokens and stable hashes", () => {
    const first = createAgentToken();
    const second = createAgentToken();

    expect(first.token).toMatch(/^ghost_agent_[A-Za-z0-9_-]{43}$/);
    expect(first.token).not.toBe(second.token);
    expect(first.tokenHash).toBe(hashAgentToken(first.token));
    expect(first.tokenHash).toHaveLength(64);
    expect(first.tokenHint).toBe(`ghost_agent_…${first.token.slice(-6)}`);
    expect(first.tokenHash).not.toContain(first.token.slice(-6));
  });
});
