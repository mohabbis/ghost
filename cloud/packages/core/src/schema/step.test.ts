import { describe, expect, it } from "vitest";
import { parseWorkflowSteps, workflowStep } from "./step.js";

describe("workflow step schema", () => {
  it("parses a valid multi-step workflow", () => {
    const steps = parseWorkflowSteps([
      { id: "a", type: "navigate", url: "https://example.com/login" },
      { id: "b", type: "fill", selector: { name: "Email" }, value: "a@b.com" },
      { id: "c", type: "click", selector: { role: "button", name: "Sign in" } },
      {
        id: "d",
        type: "verify",
        assertion: { kind: "url", expected: "https://example.com/dashboard" },
      },
    ]);
    expect(steps).toHaveLength(4);
  });

  it("defaults fill.sensitive to false", () => {
    const parsed = workflowStep.parse({
      id: "x",
      type: "fill",
      selector: { css: "#name" },
      value: "Ada",
    });
    expect(parsed.type === "fill" && parsed.sensitive).toBe(false);
  });

  it("rejects an unknown step type", () => {
    expect(() => workflowStep.parse({ id: "x", type: "teleport" })).toThrow();
  });

  it("rejects a navigate step with a non-URL", () => {
    expect(() => workflowStep.parse({ id: "x", type: "navigate", url: "not-a-url" })).toThrow();
  });

  it("rejects a step missing its id", () => {
    expect(() =>
      workflowStep.parse({ type: "click", selector: { css: "#go" } }),
    ).toThrow();
  });
});
