import { describe, expect, it } from "vitest";
import type { WorkflowStep } from "@ghost/core/schema/step";
import { shouldCaptureScreenshot } from "./driver.js";

describe("shouldCaptureScreenshot", () => {
  it("captures ordinary fills", () => {
    const step = {
      id: "1",
      type: "fill",
      selector: { role: "textbox", name: "Full name" },
      value: "Ada",
      sensitive: false,
    } satisfies WorkflowStep;
    expect(shouldCaptureScreenshot(step)).toBe(true);
  });

  it("skips when the author marked the fill sensitive", () => {
    const step = {
      id: "1",
      type: "fill",
      selector: { role: "textbox", name: "Full name" },
      value: "secret",
      sensitive: true,
    } satisfies WorkflowStep;
    expect(shouldCaptureScreenshot(step)).toBe(false);
  });

  it("skips when the classifier gates a password-shaped field even without the flag", () => {
    const step = {
      id: "1",
      type: "fill",
      selector: { role: "textbox", name: "Password" },
      value: "hunter2",
      sensitive: false,
    } satisfies WorkflowStep;
    expect(shouldCaptureScreenshot(step)).toBe(false);
  });

  it("still captures non-fill steps", () => {
    expect(
      shouldCaptureScreenshot({
        id: "1",
        type: "click",
        selector: { role: "button", name: "Submit" },
      }),
    ).toBe(true);
  });
});
