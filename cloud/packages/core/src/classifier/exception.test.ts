import { describe, expect, it } from "vitest";
import {
  EXCEPTION_KINDS,
  classifyException,
  dispositionForKind,
  duplicateRiskFor,
  kindsForOwner,
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

describe("semantic Playwright locators route to the author, not to retry", () => {
  // apps/worker/src/browser/selector.ts resolves every *preferred* selector
  // through getByRole/getByTestId/getByText, and Playwright's call log then says
  // "waiting for getByRole(...)" — the word "locator" never appears. Matching
  // only /locator|selector/ missed exactly the selectors Ghost prefers and
  // routed a changed page to an operator as a transient blip.
  it.each([
    ['Timeout 30000ms exceeded.\nCall log:\n  - waiting for getByRole(\'button\', { name: \'Pay\' })', "getByRole"],
    ['Timeout 30000ms exceeded.\nCall log:\n  - waiting for getByTestId(\'submit\')', "getByTestId"],
    ['Timeout 30000ms exceeded.\nCall log:\n  - waiting for getByText(\'Continue\')', "getByText"],
    ['Timeout 5000ms exceeded.\nCall log:\n  - waiting for getByLabel(\'Amount\')', "getByLabel"],
    ['Timeout 5000ms exceeded.\nCall log:\n  - waiting for getByPlaceholder(\'Search\')', "getByPlaceholder"],
  ])("files a %s timeout as TARGET_MISSING", (reason) => {
    expect(kindOf(reason, click)).toBe("TARGET_MISSING");
  });

  it("routes those to the author rather than telling an operator to retry", () => {
    const d = classifyException({
      reason: "Timeout 30000ms exceeded.\nCall log:\n  - waiting for getByRole('button')",
      step: click,
    });
    expect(d.owner).toBe("author");
    expect(d.retryUseful).toBe(false);
  });
});

describe("duplicateRiskFor", () => {
  it("is false when nothing is indeterminate", () => {
    expect(
      duplicateRiskFor({
        disposition: classifyException({ reason: "net::ERR_CONNECTION_RESET", step: click }),
        storedKind: "TRANSIENT",
        recordedOutcome: "FAILED",
        step: click,
      }),
    ).toBe(false);
  });

  it("is true for an indeterminate mutating step", () => {
    expect(
      duplicateRiskFor({ storedKind: "OUTCOME_UNKNOWN", step: click }),
    ).toBe(true);
  });

  it("stays FALSE for an indeterminate read, however the signal arrives", () => {
    // The regression this helper exists for: three call sites each wrote
    // `kind === "OUTCOME_UNKNOWN" || recorded === "UNKNOWN"`, which forced the
    // duplicate-effect prompt on for a `verify` — a read that costs nothing to
    // repeat. A prompt that fires on reads gets clicked through.
    expect(duplicateRiskFor({ storedKind: "OUTCOME_UNKNOWN", step: verify })).toBe(false);
    expect(duplicateRiskFor({ recordedOutcome: "UNKNOWN", step: verify })).toBe(false);
    expect(
      duplicateRiskFor({
        disposition: classifyException({ reason: "OUTCOME_UNKNOWN: ?", step: verify }),
        step: verify,
      }),
    ).toBe(false);
  });

  it("lets a stale stored kind raise, but never lower, the risk", () => {
    // Stored label says benign, recorded outcome says indeterminate: the union
    // must still warn.
    expect(
      duplicateRiskFor({ storedKind: "TRANSIENT", recordedOutcome: "UNKNOWN", step: click }),
    ).toBe(true);
    // And the reverse — a stale OUTCOME_UNKNOWN label on a read stays quiet,
    // because the step, not the label, decides whether an effect could repeat.
    expect(
      duplicateRiskFor({ storedKind: "OUTCOME_UNKNOWN", recordedOutcome: "FAILED", step: verify }),
    ).toBe(false);
  });

  it("assumes risk when the step is unknown", () => {
    expect(duplicateRiskFor({ storedKind: "OUTCOME_UNKNOWN" })).toBe(true);
  });
});

describe("a failed verification on a mutating step is duplicate risk", () => {
  // The action ran — that is *why* there was something to assert — and the
  // incident route's retry resets the step to PENDING and re-executes the whole
  // step under the original approval. For a click on Pay that is a second
  // payment, and nothing was warning about it. Stronger than uncertainty: here
  // the effect is known to have landed.
  it("flags a mutating step whose verification failed", () => {
    expect(
      duplicateRiskFor({
        disposition: classifyException({ reason: "verification failed at step 3", step: click }),
        step: click,
      }),
    ).toBe(true);
  });

  it("does not flag a verification failure on a read", () => {
    expect(
      duplicateRiskFor({
        disposition: classifyException({ reason: "verification failed at step 3", step: verify }),
        step: verify,
      }),
    ).toBe(false);
  });

  it("flags it from the stored kind too", () => {
    expect(duplicateRiskFor({ storedKind: "VERIFICATION", step: click })).toBe(true);
  });

  it("still leaves genuinely safe kinds unflagged", () => {
    for (const kind of ["TRANSIENT", "TARGET_MISSING", "AUTH", "DATA", "UNKNOWN"] as const) {
      expect(duplicateRiskFor({ storedKind: kind, step: click })).toBe(false);
    }
  });
});

describe("dispositionForKind", () => {
  it("returns that kind's own owner, headline and guidance", () => {
    const d = dispositionForKind("OUTCOME_UNKNOWN");
    expect(d.kind).toBe("OUTCOME_UNKNOWN");
    expect(d.owner).toBe("operator");
    expect(d.headline).toMatch(/already/i);
    expect(d.retryUseful).toBe(false);
  });

  it("never mixes one kind's label with another's guidance", () => {
    // The display bug this exists to prevent: a compensation incident stores an
    // explicit kind whose reason text carries no classifier prefix, so a live
    // re-classification returns UNKNOWN and the row showed "OUTCOME_UNKNOWN"
    // beside "Unclassified failure".
    for (const kind of EXCEPTION_KINDS) {
      const d = dispositionForKind(kind);
      expect(d).toEqual(
        expect.objectContaining({ kind, ...({} as Record<string, unknown>) }),
      );
      expect(d.headline).toBe(dispositionForKind(kind).headline);
      expect(d.guidance).not.toBe("");
    }
    expect(dispositionForKind("UNKNOWN").headline).not.toBe(
      dispositionForKind("OUTCOME_UNKNOWN").headline,
    );
  });
});

describe("kindsForOwner", () => {
  it("partitions every kind across the three desks exactly once", () => {
    const all = (["operator", "author", "administrator"] as const).flatMap((o) => kindsForOwner(o));
    expect(all.sort()).toEqual([...EXCEPTION_KINDS].sort());
  });

  it("puts target changes on the author's desk", () => {
    expect(kindsForOwner("author")).toContain("TARGET_MISSING");
    expect(kindsForOwner("administrator")).toEqual(
      expect.arrayContaining(["AUTH", "APPROVAL_EXPIRED"]),
    );
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
