import { describe, expect, it } from "vitest";
import {
  EXCEPTION_KINDS,
  classifyException,
  needsAuthoring,
  type ExceptionKind,
} from "./exception.js";
import type { WorkflowStep } from "../schema/step.js";

const click: WorkflowStep = {
  id: "s1",
  type: "click",
  selector: { css: "#pay" },
};

const verify: WorkflowStep = {
  id: "s2",
  type: "verify",
  assertion: { kind: "textPresent", expected: "Paid" },
};

function kindOf(reason: string, step?: WorkflowStep): ExceptionKind {
  return classifyException({ reason, step }).kind;
}

describe("classifyException — authoritative signals win", () => {
  it("treats a recorded UNKNOWN outcome as OUTCOME_UNKNOWN regardless of the reason text", () => {
    // The reason here looks like a plain transient timeout. The recorded outcome
    // is the engine's own considered judgement and must override it, or a
    // possibly-completed payment would be offered a one-click retry.
    const d = classifyException({
      reason: "Timeout 30000ms exceeded",
      step: click,
      recordedOutcome: "UNKNOWN",
    });
    expect(d.kind).toBe("OUTCOME_UNKNOWN");
    expect(d.retryMayDuplicate).toBe(true);
  });

  it("reads Ghost's own structured prefixes", () => {
    expect(
      kindOf("OUTCOME_UNKNOWN: step 4 may or may not have taken effect"),
    ).toBe("OUTCOME_UNKNOWN");
    expect(
      kindOf("RESTORE_UNSAFE: captured session digest did not match"),
    ).toBe("RESTORE_UNSAFE");
    expect(kindOf("RUN_TIMEOUT: exceeded wall-clock budget of 600000ms")).toBe(
      "TRANSIENT",
    );
  });

  it("reads the unprefixed phrases the engine writes", () => {
    expect(kindOf("approval for step 3 expired before it was used")).toBe(
      "APPROVAL_EXPIRED",
    );
    expect(kindOf("verification failed at step 7")).toBe("VERIFICATION");
  });

  it("does not sweep up unrelated errors that merely mention approval", () => {
    // Whole-phrase anchoring, not keyword matching: this is a missing button on
    // an approvals page, which is an authoring problem, not an expired gate.
    expect(kindOf('waiting for locator("#approval-banner")')).toBe(
      "TARGET_MISSING",
    );
  });
});

describe("classifyException — best-effort text rules", () => {
  it("files a Playwright locator timeout as a target change, not a blip", () => {
    // Regression guard for rule ordering: this string contains "Timeout", so a
    // generic timeout rule placed first would misfile every changed selector as
    // transient and retry it forever.
    const reason =
      'Timeout 30000ms exceeded.\nCall log:\n  - waiting for locator("#submit-order")';
    expect(kindOf(reason)).toBe("TARGET_MISSING");
  });

  it("files strict-mode violations with the author too", () => {
    expect(
      kindOf("strict mode violation: locator resolved to 3 elements"),
    ).toBe("TARGET_MISSING");
  });

  it("recognizes auth refusals", () => {
    expect(kindOf("Request failed with status 403")).toBe("AUTH");
    expect(kindOf("Your session has expired, please sign in again")).toBe(
      "AUTH",
    );
  });

  it("recognizes rejected values", () => {
    expect(kindOf("Invalid value for field 'amount'")).toBe("DATA");
    expect(kindOf("Purchase order number is required")).toBe("DATA");
  });

  it("recognizes network and lifecycle noise as transient", () => {
    expect(
      kindOf("net::ERR_CONNECTION_RESET at https://portal.example.com"),
    ).toBe("TRANSIENT");
    expect(kindOf("Target page, context or browser has been closed")).toBe(
      "TRANSIENT",
    );
    expect(kindOf("Request failed with status 503")).toBe("TRANSIENT");
  });

  it("falls back to a bare generic timeout as transient", () => {
    expect(kindOf("Timeout 5000ms exceeded")).toBe("TRANSIENT");
  });
});

describe("classifyException — fails closed", () => {
  it("leaves an unrecognized reason UNKNOWN rather than guessing", () => {
    const d = classifyException({
      reason: "the frobnicator disagreed",
      step: click,
    });
    expect(d.kind).toBe("UNKNOWN");
    expect(d.retryUseful).toBe(false);
  });

  it("does not let the bare retry-exhausted wrapper imply a category", () => {
    // This string carries no information about cause. Classifying it as
    // transient would offer a retry on a step that has already exhausted its
    // retries for an unknown reason.
    expect(kindOf("step exhausted its retries")).toBe("UNKNOWN");
  });

  it("treats an empty reason as UNKNOWN", () => {
    expect(kindOf("")).toBe("UNKNOWN");
  });

  it("assumes duplicate risk when the step is not known", () => {
    // No step means the classifier cannot prove the effect was confined to the
    // browser, so it must assume the dangerous case.
    const d = classifyException({ reason: "OUTCOME_UNKNOWN: gone dark" });
    expect(d.retryMayDuplicate).toBe(true);
  });
});

describe("retryMayDuplicate is the conjunction, not a synonym for the kind", () => {
  it("warns on an indeterminate mutating step", () => {
    const d = classifyException({ reason: "OUTCOME_UNKNOWN: ?", step: click });
    expect(d.retryMayDuplicate).toBe(true);
  });

  it("does not warn on an indeterminate read", () => {
    // Repeating a `verify` costs nothing. If this warned, the warning would
    // appear often enough to be ignored when it matters.
    const d = classifyException({ reason: "OUTCOME_UNKNOWN: ?", step: verify });
    expect(d.retryMayDuplicate).toBe(false);
  });

  it("never warns for kinds other than OUTCOME_UNKNOWN", () => {
    for (const reason of [
      "net::ERR_CONNECTION_RESET",
      'waiting for locator("#x")',
      "verification failed at step 2",
      "Request failed with status 403",
    ]) {
      expect(classifyException({ reason, step: click }).retryMayDuplicate).toBe(
        false,
      );
    }
  });
});

describe("dispositions are complete and coherent", () => {
  it("gives every kind an owner, headline, and guidance", () => {
    for (const kind of EXCEPTION_KINDS) {
      // Reach each kind through the classifier where a reason exists for it,
      // and assert the table itself is filled in for all of them.
      const d = classifyException({ reason: `${kind}:` });
      expect(d.headline.length).toBeGreaterThan(0);
      expect(d.guidance.length).toBeGreaterThan(0);
      expect(["operator", "author", "administrator"]).toContain(d.owner);
    }
  });

  it("routes target changes to the author and auth to the administrator", () => {
    expect(needsAuthoring("TARGET_MISSING")).toBe(true);
    expect(needsAuthoring("TRANSIENT")).toBe(false);
    expect(classifyException({ reason: "403 Forbidden" }).owner).toBe(
      "administrator",
    );
    expect(
      classifyException({ reason: "approval for step 1 expired" }).owner,
    ).toBe("administrator");
  });

  it("only claims retry is useful for transient failures", () => {
    // Every other kind recurs identically on retry. If this list grows, the
    // guidance text for the new kind needs to justify it.
    const useful = EXCEPTION_KINDS.filter(
      (k) => classifyException({ reason: syntheticReasonFor(k) }).kind === k,
    ).filter(
      (k) => classifyException({ reason: syntheticReasonFor(k) }).retryUseful,
    );
    expect(useful).toEqual(["TRANSIENT"]);
  });
});

/** A reason string that classifies to the given kind, for table-driven tests. */
function syntheticReasonFor(kind: ExceptionKind): string {
  switch (kind) {
    case "TRANSIENT":
      return "RUN_TIMEOUT: exceeded budget";
    case "TARGET_MISSING":
      return 'waiting for locator("#x")';
    case "AUTH":
      return "403 Forbidden";
    case "VERIFICATION":
      return "verification failed at step 1";
    case "OUTCOME_UNKNOWN":
      return "OUTCOME_UNKNOWN: dark";
    case "APPROVAL_EXPIRED":
      return "approval for step 1 expired before it was used";
    case "RESTORE_UNSAFE":
      return "RESTORE_UNSAFE: digest mismatch";
    case "DATA":
      return "Invalid value for field 'x'";
    case "UNKNOWN":
      return "no idea";
  }
}
