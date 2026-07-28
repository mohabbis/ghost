import { describe, expect, it } from "vitest";
import {
  AGENT_TOOLS,
  FORBIDDEN_AGENT_TOOLS,
  agentToolsCatalog,
  getAgentTool,
  isForbiddenAgentTool,
} from "./tools.js";

describe("agent tool catalog", () => {
  it("exposes only the allow-listed tools", () => {
    expect(AGENT_TOOLS.map((t) => t.name)).toEqual([
      "list_workflows",
      "get_workflow",
      "start_run",
      "get_run",
      "list_pending_approvals",
    ]);
  });

  it("never offers approve/reject tools", () => {
    for (const t of AGENT_TOOLS) {
      expect(isForbiddenAgentTool(t.name)).toBe(false);
      expect(t.name).not.toMatch(/approve|reject/i);
    }
    for (const name of FORBIDDEN_AGENT_TOOLS) {
      expect(getAgentTool(name)).toBeUndefined();
      expect(isForbiddenAgentTool(name)).toBe(true);
    }
  });

  it("classifies start_run as local-mutate and reads as safe-read", () => {
    expect(getAgentTool("start_run")?.riskClass).toBe("local-mutate");
    expect(getAgentTool("list_workflows")?.riskClass).toBe("safe-read");
    expect(getAgentTool("get_run")?.riskClass).toBe("safe-read");
    expect(getAgentTool("list_pending_approvals")?.riskClass).toBe("safe-read");
  });

  it("catalog documents the human-approval contract", () => {
    const catalog = agentToolsCatalog();
    expect(catalog.contract).toMatch(/human approval/i);
    expect(catalog.forbidden).toContain("approve_run");
    expect(catalog.tools.some((t) => t.name === "start_run")).toBe(true);
  });
});
