import { describe, expect, it } from "vitest";
import { interpolate, resolveStep, hasExpression, ExpressionError, EMPTY_SCOPE } from "./expr.js";
import type { WorkflowStep } from "./schema/step.js";

const scope = {
  steps: { total: { amount: "250.00", currency: "GBP" }, who: { customer: "Ada Lovelace" } },
};

describe("interpolate", () => {
  it("resolves a step output reference", () => {
    expect(interpolate("{{ steps.total.amount }}", scope)).toBe("250.00");
  });

  it("resolves several references in one string", () => {
    expect(interpolate("{{ steps.total.amount }} {{ steps.total.currency }}", scope)).toBe(
      "250.00 GBP",
    );
  });

  it("leaves a plain string alone", () => {
    expect(interpolate("no placeholders here", scope)).toBe("no placeholders here");
    expect(hasExpression("no placeholders here")).toBe(false);
  });

  it("throws on an unresolved reference rather than substituting empty", () => {
    // Quietly turning an unresolved amount into "" is exactly the class of bug
    // this product exists to prevent.
    expect(() => interpolate("{{ steps.total.vat }}", scope)).toThrow(/no value named "vat"/);
    expect(() => interpolate("{{ steps.nope.amount }}", scope)).toThrow(/step "nope"/);
  });

  // --- It is deliberately not a language ------------------------------------

  it("does not evaluate code", () => {
    expect(() => interpolate("{{ 1 + 1 }}", scope)).toThrow(ExpressionError);
    expect(() => interpolate("{{ process.env.SECRET }}", scope)).toThrow(ExpressionError);
    expect(() => interpolate("{{ steps.who.customer.toUpperCase() }}", scope)).toThrow(ExpressionError);
    expect(() => interpolate("{{ steps.total.amount; drop() }}", scope)).toThrow(ExpressionError);
  });

  it("rejects property walks beyond the two accepted shapes", () => {
    expect(() => interpolate("{{ steps.total }}", scope)).toThrow(ExpressionError);
    expect(() => interpolate("{{ steps.total.amount.length }}", scope)).toThrow(ExpressionError);
    expect(() => interpolate("{{ steps }}", scope)).toThrow(ExpressionError);
  });

  it("does not re-interpolate a resolved value", () => {
    // A captured value that happens to contain braces must be inserted as
    // data, never re-parsed as an expression.
    const hostile = { steps: { s: { evil: "{{ steps.total.amount }}" } } };
    expect(interpolate("{{ steps.s.evil }}", hostile)).toBe("{{ steps.total.amount }}");
  });

  it("rejects the vars form, which no longer exists", () => {
    // Removed rather than left failing: nothing populates run inputs yet, so
    // every use of it was a guaranteed error against a documented syntax.
    expect(() => interpolate("{{ vars.customer }}", scope)).toThrow(/only \{\{ steps/);
  });
});

describe("resolveStep", () => {
  it("resolves the value of a fill", () => {
    const step: WorkflowStep = {
      id: "amt",
      type: "fill",
      selector: { name: "Amount" },
      value: "{{ steps.total.amount }}",
      sensitive: false,
    };
    const resolved = resolveStep(step, scope);
    expect(resolved.type === "fill" && resolved.value).toBe("250.00");
  });

  it("resolves a navigate URL", () => {
    const step: WorkflowStep = {
      id: "go",
      type: "navigate",
      url: "https://example.com/{{ steps.who.customer }}",
    };
    const resolved = resolveStep(step, scope);
    expect(resolved.type === "navigate" && resolved.url).toContain("Ada Lovelace");
  });

  it("leaves selectors untouched", () => {
    // Letting a captured value choose which element to click would make the
    // reviewed plan and the executed action come apart — the same failure
    // mode as allowing arbitrary code.
    const step: WorkflowStep = {
      id: "c",
      type: "click",
      selector: { role: "button", name: "{{ steps.who.customer }}" },
    };
    const resolved = resolveStep(step, scope);
    expect(resolved.type === "click" && resolved.selector.name).toBe("{{ steps.who.customer }}");
  });

  it("passes through steps with nothing to resolve", () => {
    const step: WorkflowStep = { id: "a", type: "approval", reason: "check totals" };
    expect(resolveStep(step, EMPTY_SCOPE)).toEqual(step);
  });
});
