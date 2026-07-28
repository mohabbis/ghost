import { describe, expect, it } from "vitest";
import {
  classifyStep,
  sensitiveStepIndices,
  workflowHasSensitiveStep,
} from "./sensitive.js";
import type { WorkflowStep } from "../schema/step.js";

const step = (s: WorkflowStep) => s;

describe("classifyStep", () => {
  it("does not gate a plain navigation", () => {
    expect(
      classifyStep(step({ id: "1", type: "navigate", url: "https://example.com" })).sensitive,
    ).toBe(false);
  });

  it("does not gate a read-only extract", () => {
    expect(
      classifyStep(
        step({ id: "1", type: "extract", selector: { css: ".total" }, name: "total" }),
      ).sensitive,
    ).toBe(false);
  });

  it("gates a click on a Submit button", () => {
    const v = classifyStep(
      step({ id: "1", type: "click", selector: { role: "button", name: "Submit order" } }),
    );
    expect(v.sensitive).toBe(true);
    expect(v.reason).toMatch(/submit/i);
  });

  it("gates a click on a Delete control by description", () => {
    const v = classifyStep(
      step({
        id: "1",
        type: "click",
        selector: { css: ".x" },
        description: "Delete the customer record",
      }),
    );
    expect(v.sensitive).toBe(true);
  });

  it("does not gate a click on a benign Next button", () => {
    expect(
      classifyStep(
        step({ id: "1", type: "click", selector: { role: "button", name: "Next" } }),
      ).sensitive,
    ).toBe(false);
  });

  it("gates a fill flagged sensitive", () => {
    expect(
      classifyStep(
        step({ id: "1", type: "fill", selector: { css: "#p" }, value: "x", sensitive: true }),
      ).sensitive,
    ).toBe(true);
  });

  it("gates a fill into a password field by name even when not flagged", () => {
    expect(
      classifyStep(
        step({
          id: "1",
          type: "fill",
          selector: { name: "Password" },
          value: "x",
          sensitive: false,
        }),
      ).sensitive,
    ).toBe(true);
  });

  it("does not gate a fill into an ordinary text field", () => {
    expect(
      classifyStep(
        step({
          id: "1",
          type: "fill",
          selector: { name: "Full name" },
          value: "Ada",
          sensitive: false,
        }),
      ).sensitive,
    ).toBe(false);
  });

  it("always gates sendEmail and apiCall", () => {
    expect(
      classifyStep(
        step({ id: "1", type: "sendEmail", connectorId: "c", to: ["a@b.com"], subject: "s", body: "b" }),
      ).sensitive,
    ).toBe(true);
    expect(
      classifyStep(step({ id: "1", type: "apiCall", connectorId: "c", operation: "createDeal" }))
        .sensitive,
    ).toBe(true);
  });

  it("treats an explicit approval step as a gate", () => {
    expect(
      classifyStep(step({ id: "1", type: "approval", reason: "double-check totals" })).sensitive,
    ).toBe(true);
  });
});

describe("workflow-level helpers", () => {
  const steps: WorkflowStep[] = [
    { id: "1", type: "navigate", url: "https://example.com" },
    { id: "2", type: "fill", selector: { name: "Full name" }, value: "Ada", sensitive: false },
    { id: "3", type: "click", selector: { role: "button", name: "Send invoice" } },
  ];

  it("detects at least one sensitive step", () => {
    expect(workflowHasSensitiveStep(steps)).toBe(true);
  });

  it("returns the indices of sensitive steps", () => {
    expect(sensitiveStepIndices(steps)).toEqual([2]);
  });
});
