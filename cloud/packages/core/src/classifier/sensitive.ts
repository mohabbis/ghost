import type { Selector, WorkflowStep } from "../schema/step.js";

/**
 * Deterministic sensitive-action classifier.
 *
 * Decides whether a workflow step must pause for human approval before it runs.
 * This is pure, rule-based logic — **no AI, no network**. AI may draft or
 * explain workflows elsewhere, but the decision to gate a mutating action is
 * deterministic and auditable, which is the whole trust proposition.
 *
 * Ported in spirit from the desktop app's Ghost Guard heuristics.
 */

export interface SensitiveVerdict {
  sensitive: boolean;
  /** Present when `sensitive` is true — why the step was gated. */
  reason?: string;
}

const NOT_SENSITIVE: SensitiveVerdict = { sensitive: false };

// Words that, on a clicked control, indicate an irreversible or outbound action.
const SENSITIVE_ACTION_WORDS = [
  "submit",
  "send",
  "pay",
  "purchase",
  "buy",
  "checkout",
  "place order",
  "confirm",
  "delete",
  "remove",
  "transfer",
  "wire",
  "approve",
  "sign",
  "publish",
  "cancel subscription",
  "deactivate",
  "archive",
];

// Field-name / label hints for secret or high-risk input.
const SENSITIVE_FIELD_WORDS = [
  "password",
  "passcode",
  "otp",
  "one-time",
  "2fa",
  "mfa",
  "cvv",
  "cvc",
  "card number",
  "credit card",
  "account number",
  "routing",
  "ssn",
  "social security",
  "pin",
];

function selectorHaystack(selector: Selector | undefined): string {
  if (!selector) return "";
  return [selector.name, selector.text, selector.testId, selector.css]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
}

function matches(haystack: string, needles: string[]): string | undefined {
  return needles.find((n) => haystack.includes(n));
}

/**
 * Classify a single step. A `true` verdict forces an approval gate at replay.
 */
export function classifyStep(step: WorkflowStep): SensitiveVerdict {
  switch (step.type) {
    // An explicit approval step is, by definition, a gate.
    case "approval":
      return { sensitive: true, reason: step.reason };

    // Outbound / mutating integration calls always gate in the MVP.
    case "sendEmail":
      return { sensitive: true, reason: "Sends an email via a connector" };
    case "apiCall":
      return {
        sensitive: true,
        reason: `Calls connector operation "${step.operation}"`,
      };

    case "click": {
      const hay = selectorHaystack(step.selector) + " " + (step.description ?? "").toLowerCase();
      const hit = matches(hay, SENSITIVE_ACTION_WORDS);
      return hit
        ? { sensitive: true, reason: `Clicks a "${hit}" control` }
        : NOT_SENSITIVE;
    }

    case "fill": {
      if (step.sensitive) {
        return { sensitive: true, reason: "Enters data into a secret field" };
      }
      const hit = matches(selectorHaystack(step.selector), SENSITIVE_FIELD_WORDS);
      return hit
        ? { sensitive: true, reason: `Enters data into a "${hit}" field` }
        : NOT_SENSITIVE;
    }

    // Reads and navigations are not gated on their own.
    case "navigate":
    case "select":
    case "waitFor":
    case "extract":
    case "verify":
      return NOT_SENSITIVE;

    default: {
      // Exhaustiveness guard: a new step type must be classified explicitly.
      const _never: never = step;
      void _never;
      return { sensitive: true, reason: "Unknown step type — gated by default" };
    }
  }
}

/**
 * True when a step's side effect must never be applied twice.
 *
 * Deliberately the *same* boundary as gating rather than a second list to keep
 * in sync: if the classifier is not trusted to decide what needs approval, it
 * is not trusted to decide what is safe to re-run either. One predicate, one
 * test suite, one thing to get right.
 *
 * The engine uses this for the crash window — a step that recorded
 * `step.started` and no terminal outcome. Re-running an at-most-once step risks
 * a double payment; skipping it risks a silent no-pay. Neither is the engine's
 * call, so the run stops and a human decides.
 */
export function isAtMostOnce(step: WorkflowStep): boolean {
  return classifyStep(step).sensitive;
}

/** Returns true if any step in the workflow requires approval. */
export function workflowHasSensitiveStep(steps: WorkflowStep[]): boolean {
  return steps.some((s) => classifyStep(s).sensitive);
}

/** Zero-based indices of every step that requires approval. */
export function sensitiveStepIndices(steps: WorkflowStep[]): number[] {
  const out: number[] = [];
  steps.forEach((s, i) => {
    if (classifyStep(s).sensitive) out.push(i);
  });
  return out;
}
