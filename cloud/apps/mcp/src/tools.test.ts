import { describe, expect, it } from "vitest";
import { AGENT_TOOLS, FORBIDDEN_AGENT_TOOLS, isForbiddenAgentTool } from "@ghost/core/agent";

describe("mcp tool surface", () => {
  it("mirrors the core allow list and never exposes approve", () => {
    const names = AGENT_TOOLS.map((t) => t.name);
    expect(names).toContain("start_run");
    expect(names).toContain("list_pending_approvals");
    for (const banned of FORBIDDEN_AGENT_TOOLS) {
      expect(names).not.toContain(banned);
      expect(isForbiddenAgentTool(banned)).toBe(true);
    }
  });
});
