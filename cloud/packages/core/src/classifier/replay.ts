import type { WorkflowStep } from "../schema/step.js";

/**
 * Deterministic replay-safety classifier.
 *
 * Sibling of `classifyStep` (sensitive.ts) and held to the same discipline:
 * pure, rule-based, exhaustive, fail-closed. Where the sensitivity classifier
 * answers *"must a human approve this before it runs?"*, this one answers
 * *"is it safe to re-apply this silently while rebuilding page state?"*
 *
 * The two are deliberately different bars. Restoration is unattended — nobody
 * is watching — so it excludes every click, including benign ones. Re-attempting
 * an interrupted benign step (which the state machine may do) is at least
 * recorded in the journal and visible in the run timeline.
 */

export type ReplaySafety =
  /** Re-establishes client-side state. Touches nothing outside the browser. */
  | "restorative"
  /** Nothing to restore — skip it. */
  | "inert"
  /** Reaches the outside world. NEVER re-applied. */
  | "mutating";

export function replaySafety(step: WorkflowStep): ReplaySafety {
  switch (step.type) {
    // Re-typing a form field or re-selecting an option reproduces client-side
    // state the user would see. `navigate` is restorative only in the sense
    // that it moves the page; the restore planner additionally requires it to
    // target the URL being restored to.
    case "navigate":
    case "fill":
    case "select":
    case "waitFor":
      return "restorative";

    // Assertions and gates produce no page state worth rebuilding, and an
    // `extract` is a read whose value the journal already holds.
    case "verify":
    case "approval":
    case "extract":
      return "inert";

    // A click can submit, pay, or delete. A connector call leaves the browser
    // entirely. Neither is ever re-applied during restoration.
    case "click":
    case "apiCall":
    case "sendEmail":
      return "mutating";

    default: {
      // Exhaustiveness guard: a new step type must be classified explicitly,
      // and until it is, it is treated as unsafe to replay.
      const _never: never = step;
      void _never;
      return "mutating";
    }
  }
}

/** True when a step must never be silently re-applied during restoration. */
export function isRestorable(step: WorkflowStep): boolean {
  return replaySafety(step) === "restorative";
}
