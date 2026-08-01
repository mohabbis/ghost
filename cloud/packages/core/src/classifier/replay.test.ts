import { describe, expect, it } from "vitest";
import { replaySafety, isRestorable } from "./replay.js";
import { workflowStep, type WorkflowStep } from "../schema/step.js";

const samples: Record<WorkflowStep["type"], WorkflowStep> = {
  navigate: { id: "n", type: "navigate", url: "https://example.com" },
  click: { id: "c", type: "click", selector: { role: "button", name: "Save" } },
  fill: { id: "f", type: "fill", selector: { name: "Name" }, value: "Ada", sensitive: false },
  select: { id: "s", type: "select", selector: { name: "Country" }, value: "UK" },
  waitFor: { id: "w", type: "waitFor", ms: 100 },
  extract: { id: "e", type: "extract", selector: { css: "#total" }, name: "total" },
  verify: { id: "v", type: "verify", assertion: { kind: "textPresent", expected: "ok" } },
  approval: { id: "a", type: "approval", reason: "check" },
  apiCall: { id: "api", type: "apiCall", connectorId: "conn", operation: "create" },
  sendEmail: { id: "m", type: "sendEmail", connectorId: "conn", to: ["a@b.c"], subject: "s", body: "b" },
};

describe("replaySafety", () => {
  it("classifies every step type", () => {
    // Iterating the schema's own members means adding a step type without
    // classifying it fails here rather than silently defaulting at runtime —
    // the same discipline as the classifier's exhaustiveness guard.
    const types = workflowStep.options.map((o) => o.shape.type.value);
    for (const type of types) {
      expect(samples[type as WorkflowStep["type"]], `no sample for "${type}"`).toBeDefined();
      expect(() => replaySafety(samples[type as WorkflowStep["type"]])).not.toThrow();
    }
  });

  it("never treats an outward-reaching action as restorative", () => {
    expect(replaySafety(samples.click)).toBe("mutating");
    expect(replaySafety(samples.apiCall)).toBe("mutating");
    expect(replaySafety(samples.sendEmail)).toBe("mutating");
  });

  it("treats client-side form state as restorative", () => {
    expect(replaySafety(samples.fill)).toBe("restorative");
    expect(replaySafety(samples.select)).toBe("restorative");
    expect(replaySafety(samples.waitFor)).toBe("restorative");
    expect(replaySafety(samples.navigate)).toBe("restorative");
  });

  it("treats reads and gates as inert", () => {
    expect(replaySafety(samples.verify)).toBe("inert");
    expect(replaySafety(samples.approval)).toBe("inert");
    expect(replaySafety(samples.extract)).toBe("inert");
  });

  it("fails closed on an unknown step type", () => {
    const bogus = { id: "?", type: "teleport" } as unknown as WorkflowStep;
    expect(replaySafety(bogus)).toBe("mutating");
    expect(isRestorable(bogus)).toBe(false);
  });
});
