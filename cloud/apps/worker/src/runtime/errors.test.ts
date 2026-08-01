import { describe, expect, it } from "vitest";
import { classifyError } from "./errors.js";
import { effectiveRetry, effectiveTimeout, backoffFor } from "./policy.js";
import type { WorkflowStep } from "@ghost/core/schema/step";

const benign: WorkflowStep = {
  id: "f",
  type: "fill",
  selector: { name: "Full name" },
  value: "Ada",
  sensitive: false,
};

const sensitive: WorkflowStep = {
  id: "pay",
  type: "click",
  selector: { role: "button", name: "Pay now" },
};

function timeout(): Error {
  const err = new Error("locator.click: Timeout 30000ms exceeded.");
  err.name = "TimeoutError";
  return err;
}

describe("classifyError", () => {
  it("retries a timeout on a benign step", () => {
    expect(classifyError(timeout(), benign)).toBe("retryable");
  });

  it("refuses to retry a timeout on an at-most-once step", () => {
    // The same exception, the opposite handling: the click may already have
    // reached the server, so re-attempting could take the payment twice.
    expect(classifyError(timeout(), sensitive)).toBe("indeterminate");
  });

  it("treats transport failures as retryable only when repeating is safe", () => {
    const net = new Error("net::ERR_CONNECTION_RESET at https://example.com");
    expect(classifyError(net, benign)).toBe("retryable");
    expect(classifyError(net, sensitive)).toBe("indeterminate");
  });

  it("treats a crashed page the same way", () => {
    const closed = new Error("Target closed");
    expect(classifyError(closed, benign)).toBe("retryable");
    expect(classifyError(closed, sensitive)).toBe("indeterminate");
  });

  it("treats authoring mistakes as terminal, even on a benign step", () => {
    expect(
      classifyError(new Error('strict mode violation: resolved to 3 elements'), benign),
    ).toBe("terminal");
    expect(
      classifyError(new Error("selector has no resolvable field"), benign),
    ).toBe("terminal");
  });

  it("fails closed on anything unrecognized", () => {
    expect(classifyError(new Error("something odd happened"), benign)).toBe("terminal");
    expect(classifyError("not even an error", benign)).toBe("terminal");
  });
});

describe("retry policy", () => {
  it("honours an authored policy on a benign step", () => {
    const withRetry = { ...benign, retry: { maxAttempts: 3, backoffMs: 500, factor: 2 } };
    expect(effectiveRetry(withRetry).maxAttempts).toBe(3);
  });

  it("clamps a sensitive step to one attempt whatever the definition says", () => {
    // A workflow authored (or edited) with retries on a payment step must not
    // be able to talk the engine into retrying it.
    const withRetry = { ...sensitive, retry: { maxAttempts: 5, backoffMs: 500, factor: 2 } };
    expect(effectiveRetry(withRetry).maxAttempts).toBe(1);
  });

  it("defaults to a single attempt", () => {
    expect(effectiveRetry(benign).maxAttempts).toBe(1);
  });

  it("grows the backoff geometrically", () => {
    const policy = { maxAttempts: 4, backoffMs: 100, factor: 2 };
    expect(backoffFor(policy, 1)).toBe(0);
    expect(backoffFor(policy, 2)).toBe(100);
    expect(backoffFor(policy, 3)).toBe(200);
    expect(backoffFor(policy, 4)).toBe(400);
  });

  it("uses the step timeout when set, else the default", () => {
    expect(effectiveTimeout({ ...benign, timeoutMs: 1234 })).toBe(1234);
    expect(effectiveTimeout(benign)).toBe(30_000);
  });
});
