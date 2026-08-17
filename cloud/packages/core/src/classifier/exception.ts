import type { WorkflowStep } from "../schema/step.js";
import { replaySafety } from "./replay.js";

/**
 * Deterministic exception classifier — the routing half of Camunda incidents.
 *
 * Third sibling of `classifyStep` (sensitive.ts) and `replaySafety` (replay.ts),
 * held to the same discipline: pure, rule-based, exhaustive, fail-closed. The
 * three answer different questions about the same step:
 *
 * | Classifier      | Question |
 * |---|---|
 * | `classifyStep`  | Must a human approve this before it runs? |
 * | `replaySafety`  | Is it safe to re-apply silently while restoring state? |
 * | `classifyException` | This stopped. Who fixes it, and is retrying safe? |
 *
 * ## Why this is not a model call
 *
 * Engineering rule 1 — AI may propose, deterministic code executes. An incident
 * disposition decides whether a human is *offered* a one-click retry on a step
 * that may already have charged a customer. A model that classifies that wrong
 * once has caused a double payment, so the mapping is a rule table that can be
 * read, tested, and argued with.
 *
 * ## Two tiers of signal, and why the order matters
 *
 * 1. **Authoritative.** The recorded step outcome, and the structured prefixes
 *    Ghost itself emits (`OUTCOME_UNKNOWN:`, `RESTORE_UNSAFE:`, `RUN_TIMEOUT:`).
 *    These mean exactly what they say.
 * 2. **Best-effort.** Raw driver text from Playwright or a target system. Useful
 *    — "waiting for locator" really does mean the page changed — but it is a
 *    third party's prose and may be reworded by an upgrade at any time.
 *
 * Tier 1 is checked first and wins outright. Tier 2 only ever refines an
 * otherwise-`UNKNOWN` result, and an unrecognized reason stays `UNKNOWN` rather
 * than being forced into the nearest-looking bucket. A wrong-but-confident label
 * on an incident is worse than no label, because the whole point is telling a
 * human where to look.
 */

export type ExceptionKind =
  /** Infrastructure or timing. Nothing is wrong with the workflow. */
  | "TRANSIENT"
  /** The element is gone. The target UI changed under a recorded workflow. */
  | "TARGET_MISSING"
  /** Credentials, session, or permission at the target system. */
  | "AUTH"
  /** The step ran and the outcome was not what the workflow asserted. */
  | "VERIFICATION"
  /** The step may or may not have taken effect. Retrying may repeat it. */
  | "OUTCOME_UNKNOWN"
  /** An approval was granted but expired before the run could consume it. */
  | "APPROVAL_EXPIRED"
  /** Page state could not be provably rebuilt, so the run refused to guess. */
  | "RESTORE_UNSAFE"
  /** A value was rejected by the target system. */
  | "DATA"
  /** Unrecognized. Needs human judgement — never assumed benign. */
  | "UNKNOWN";

/**
 * Which desk an exception belongs on.
 *
 * This is the actual routing decision, and it is why this file exists rather
 * than a label on a status badge. "A run stopped" is not actionable; "the portal
 * changed and whoever maintains this workflow needs to re-record step 4" is.
 */
export type ExceptionOwner =
  /** The person running the work. Can decide, retry, or escalate. */
  | "operator"
  /** Whoever maintains this workflow. The steps themselves need changing. */
  | "author"
  /** Whoever holds credentials and grants approvals for this org. */
  | "administrator";

export interface ExceptionDisposition {
  kind: ExceptionKind;
  /** Which desk this belongs on. */
  owner: ExceptionOwner;
  /** One line naming the class of problem, safe to show in a queue row. */
  headline: string;
  /** What the owner should actually do. */
  guidance: string;
  /**
   * Would retrying the same step plausibly change the outcome?
   *
   * False does not mean "forbidden" — a human may always retry, and the engine
   * deliberately lets them (see journal.ts on clearing `inFlight`). It means the
   * UI should not lead with retry, because the same failure will recur.
   */
  retryUseful: boolean;
  /**
   * Could retrying repeat an effect that has already happened?
   *
   * The one field with teeth. When true, a retry needs explicit human
   * acknowledgement rather than a one-click button, and that acknowledgement is
   * recorded — the difference between an informed decision and an accident.
   */
  retryMayDuplicate: boolean;
}

/** Authoritative prefixes emitted by Ghost's own engine. */
const PREFIX_KINDS: ReadonlyArray<readonly [string, ExceptionKind]> = [
  ["OUTCOME_UNKNOWN:", "OUTCOME_UNKNOWN"],
  ["RESTORE_UNSAFE:", "RESTORE_UNSAFE"],
  ["RUN_TIMEOUT:", "TRANSIENT"],
];

/**
 * Best-effort patterns over third-party error text, most specific first.
 *
 * Ordering is load-bearing: a Playwright locator timeout contains the word
 * "Timeout", so `TARGET_MISSING` must be tested before the generic timeout rule
 * or every changed selector would be misfiled as a transient blip and retried
 * forever.
 */
const TEXT_RULES: ReadonlyArray<readonly [RegExp, ExceptionKind]> = [
  // Playwright's locator failures. "strict mode violation" is the opposite
  // problem — too many matches — but lands on the same desk: the selector no
  // longer identifies one thing, so the workflow needs re-authoring.
  //
  // The `getBy*` forms are not optional extras: apps/worker/src/browser/selector.ts
  // resolves every *preferred* selector through `getByRole`/`getByTestId`/
  // `getByText`, and Playwright's call log then reads "waiting for
  // getByRole(...)" — the word "locator" never appears. Matching only
  // `locator|selector` therefore missed exactly the selectors Ghost prefers,
  // dropping them through to the generic timeout rule below and routing a
  // changed page to an operator as a transient blip to retry forever. The rest
  // of the family is listed too, so a workflow that starts using them is not a
  // silent regression.
  [
    /waiting for (?:locator|selector|getBy(?:Role|TestId|Text|Label|Placeholder|AltText|Title))|strict mode violation|no element matches/i,
    "TARGET_MISSING",
  ],
  [
    /element is not (?:visible|attached|enabled)|not attached to the DOM/i,
    "TARGET_MISSING",
  ],

  // Auth before generic HTTP, since a 403 body often also mentions the URL.
  [/\b(?:401|403)\b|unauthorized|forbidden|access denied/i, "AUTH"],
  [
    /session (?:has )?expired|please (?:sign|log) ?in|login required|invalid credentials/i,
    "AUTH",
  ],

  // Value rejected by the target, as opposed to the field not being found.
  [/\b(?:invalid|malformed)\b.*\b(?:value|format|input|field)\b/i, "DATA"],
  [
    /is required\b|must be (?:a|an|at least|no more)|does not match the expected/i,
    "DATA",
  ],

  // Network and browser-lifecycle noise. Genuinely transient.
  [
    /net::ERR_|ECONNREFUSED|ECONNRESET|ETIMEDOUT|EAI_AGAIN|socket hang up/i,
    "TRANSIENT",
  ],
  [
    /target (?:page|closed|crashed)|browser has been closed|page crashed/i,
    "TRANSIENT",
  ],
  [
    /\b(?:429|502|503|504)\b|too many requests|service unavailable|bad gateway/i,
    "TRANSIENT",
  ],

  // Generic timeout last, after every specific timeout shape above.
  [/timeout .* exceeded|timed out/i, "TRANSIENT"],
];

const DISPOSITIONS: Record<
  ExceptionKind,
  Omit<ExceptionDisposition, "kind" | "retryMayDuplicate">
> = {
  TRANSIENT: {
    owner: "operator",
    headline: "Temporary failure",
    guidance:
      "Infrastructure or timing, not the workflow. Retry the step; if it fails the same way twice, treat it as a target change instead.",
    retryUseful: true,
  },
  TARGET_MISSING: {
    owner: "author",
    headline: "Target element not found",
    guidance:
      "The page no longer matches what was recorded. Retrying will fail identically — re-record or edit the step's selector, then start a new run.",
    retryUseful: false,
  },
  AUTH: {
    owner: "administrator",
    headline: "Authentication or permission refused",
    guidance:
      "The target system rejected Ghost's session or permissions. Refresh the stored credential or widen its scope, then retry.",
    retryUseful: false,
  },
  VERIFICATION: {
    owner: "operator",
    headline: "Outcome did not match expectation",
    guidance:
      "The action ran but the result was not what the workflow asserted. Check the target system: if the outcome is actually acceptable, skip the check; if not, the run did the wrong thing and should be reversed.",
    retryUseful: false,
  },
  OUTCOME_UNKNOWN: {
    owner: "operator",
    headline: "Effect unknown — may already have happened",
    guidance:
      "The step started and never reported back. Ghost will not guess. Confirm in the target system whether it took effect before deciding: retrying could repeat it.",
    retryUseful: false,
  },
  APPROVAL_EXPIRED: {
    owner: "administrator",
    headline: "Approval expired before use",
    guidance:
      "Someone approved this step but the run could not consume it in time. A fresh approval is required — the expired one cannot be reused.",
    retryUseful: false,
  },
  RESTORE_UNSAFE: {
    owner: "operator",
    headline: "Could not rebuild page state",
    guidance:
      "Resuming would have meant guessing at browser state, so the run stopped instead. Start a fresh run rather than retrying from here.",
    retryUseful: false,
  },
  DATA: {
    owner: "operator",
    headline: "Value rejected by the target",
    guidance:
      "The target system refused a value this run supplied. Retrying sends the same value — correct the input or the extracting step first.",
    retryUseful: false,
  },
  UNKNOWN: {
    owner: "operator",
    headline: "Unclassified failure",
    guidance:
      "Ghost could not categorize this failure. Read the error and the screenshot before acting; treat the step's effect as uncertain.",
    retryUseful: false,
  },
};

export interface ClassifyExceptionInput {
  /** The incident reason recorded on the run. */
  reason: string;
  /** The step the run stopped on, when it is known. */
  step?: WorkflowStep;
  /**
   * The recorded outcome of that step, when known.
   *
   * `"UNKNOWN"` is authoritative and overrides the reason text: the engine only
   * writes it after deciding an action's effect is genuinely indeterminate.
   */
  recordedOutcome?: "UNKNOWN" | "FAILED" | null;
}

export function classifyException(
  input: ClassifyExceptionInput,
): ExceptionDisposition {
  const kind = classifyKind(input);
  const base = DISPOSITIONS[kind];

  // Duplicate risk is the conjunction of two independent facts: the engine does
  // not know whether the effect happened, AND the step is one whose effect
  // reaches outside the browser. An indeterminate `verify` or `extract` is a
  // read — repeating it costs nothing — so it must not carry the same warning
  // as an indeterminate payment, or the warning stops meaning anything.
  //
  // Composing `replaySafety` here rather than re-listing step types keeps one
  // definition of "mutating" across restoration and incident recovery.
  const mutating = input.step ? replaySafety(input.step) === "mutating" : true;
  const retryMayDuplicate = kind === "OUTCOME_UNKNOWN" && mutating;

  return { kind, ...base, retryMayDuplicate };
}

function classifyKind(input: ClassifyExceptionInput): ExceptionKind {
  // Tier 1a: the recorded outcome. The engine already made this judgement with
  // more context than a string match will ever have.
  if (input.recordedOutcome === "UNKNOWN") return "OUTCOME_UNKNOWN";

  const reason = input.reason ?? "";

  // Tier 1b: Ghost's own structured prefixes.
  for (const [prefix, kind] of PREFIX_KINDS) {
    if (reason.startsWith(prefix)) return kind;
  }

  // Phrases the engine writes without a prefix. Matched as whole phrases rather
  // than keywords so an unrelated error mentioning "approval" is not swept up.
  if (/^approval for step \d+ expired/i.test(reason)) return "APPROVAL_EXPIRED";
  if (/^verification failed at step \d+/i.test(reason)) return "VERIFICATION";

  // "exhausted its retries" is the wrapper the step loop uses when it has no
  // better message; on its own it says nothing about *why*, so it must not
  // short-circuit the text rules below — the underlying driver error, when the
  // loop managed to capture one, is the informative part.
  const bare = /^step exhausted its retries$/i.test(reason);
  if (bare) return "UNKNOWN";

  // Tier 2: best-effort over third-party text.
  for (const [pattern, kind] of TEXT_RULES) {
    if (pattern.test(reason)) return kind;
  }

  return "UNKNOWN";
}

/**
 * Duplicate-effect risk for a stopped step, as a standalone decision.
 *
 * Exported because three read paths (the retry gate, the run timeline, and the
 * exception queue) all need it, and each had been re-deriving it as
 * `disposition.retryMayDuplicate || kind === "OUTCOME_UNKNOWN" || recorded ===
 * "UNKNOWN"`. That looked like belt-and-braces caution and was actually a bug:
 * it forced the warning on for an indeterminate **read** — a `verify` or
 * `extract` — which `classifyException` deliberately reports as safe, because
 * repeating a read costs nothing. A confirmation prompt that fires on reads is
 * one operators learn to click through, which is precisely how the prompt stops
 * protecting the payment it exists for.
 *
 * The caution those callers wanted is real, though: a *stored* `incidentKind`
 * must never be able to talk the risk down. So this takes the union of the
 * live verdict and the stored label, and then — the part the callers dropped —
 * still requires the step to actually reach outside the browser.
 *
 * Fails closed: an unknown step is assumed mutating.
 */
export function duplicateRiskFor(input: {
  /** Live classifier verdict, when already computed. */
  disposition?: ExceptionDisposition;
  /** The stored `Run.incidentKind`, which may be stale or absent. */
  storedKind?: ExceptionKind | string | null;
  /** Recorded outcome of the stopped step. */
  recordedOutcome?: "UNKNOWN" | "FAILED" | null;
  /** The stopped-on step, when known. */
  step?: WorkflowStep;
}): boolean {
  const indeterminate =
    input.disposition?.kind === "OUTCOME_UNKNOWN" ||
    input.storedKind === "OUTCOME_UNKNOWN" ||
    input.recordedOutcome === "UNKNOWN";
  if (!indeterminate) return false;
  return input.step ? replaySafety(input.step) === "mutating" : true;
}

/** Every kind, for exhaustive UI rendering and tests. */
export const EXCEPTION_KINDS = Object.keys(
  DISPOSITIONS,
) as ReadonlyArray<ExceptionKind>;

/** True when this kind is a defect in the workflow rather than in the world. */
export function needsAuthoring(kind: ExceptionKind): boolean {
  return DISPOSITIONS[kind].owner === "author";
}

/**
 * Every kind that routes to a given desk.
 *
 * `owner` is a pure function of `kind`, so an owner filter can be pushed into
 * SQL as `incidentKind IN (...)` instead of being applied to an
 * already-capped page of rows — which silently returned "no author-owned
 * exceptions" whenever the oldest N happened to be operator-owned.
 */
export function kindsForOwner(owner: ExceptionOwner): ExceptionKind[] {
  return EXCEPTION_KINDS.filter((k) => DISPOSITIONS[k].owner === owner);
}
