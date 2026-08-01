import { classifyStep } from "../classifier/sensitive.js";
import type { Compensation, CompensationAction, WorkflowStep } from "../schema/step.js";
import type { RunJournal } from "./journal.js";

/**
 * Plan the reversal of a run — Ghost's take on the BPMN saga.
 *
 * Camunda triggers compensation by invoking the handlers of completed
 * activities in reverse order. The run journal makes the same thing tractable
 * here: it already records exactly which steps completed, so reversal is a
 * backwards walk over that record rather than a guess.
 *
 * Two properties this planner is built around:
 *
 * **It is honest about what it cannot undo.** A completed step with no
 * `compensate` block appears in the plan as `irreversible`, not omitted. A
 * partial undo presented as a complete one would be worse than no undo at all —
 * an operator needs to know that the refund happened but the confirmation email
 * cannot be unsent.
 *
 * **Reversal is not automatically safe.** Undoing a payment is itself a
 * mutating, outbound action. Every compensation is classified through the same
 * `classifyStep` used for forward execution, and a sensitive one gates.
 *
 * Pure: no Prisma, no Playwright.
 */

export interface CompensationEntry {
  /** Index of the forward step being reversed. */
  stepIndex: number;
  stepId: string;
  stepLabel: string;
  compensation: Compensation;
  /** True when reversing this step needs human approval before it runs. */
  requiresApproval: boolean;
  approvalReason?: string;
}

export interface IrreversibleEntry {
  stepIndex: number;
  stepId: string;
  stepLabel: string;
  /** Why it cannot be reversed, in words an operator can act on. */
  reason: string;
}

export interface CompensationPlan {
  /** Reversals to perform, in the order they must run (reverse of execution). */
  entries: CompensationEntry[];
  /** Completed steps that cannot be reversed. Reported, never hidden. */
  irreversible: IrreversibleEntry[];
  /** True when every completed side effect can be reversed. */
  complete: boolean;
}

/**
 * Build the reversal plan for a run.
 *
 * `journal.completed` is the authority on what actually happened — the same
 * fold that drives forward execution, so a step that never ran is never
 * "undone" and a step that ran is never missed.
 */
export function planCompensation(
  steps: readonly WorkflowStep[],
  journal: RunJournal,
): CompensationPlan {
  const entries: CompensationEntry[] = [];
  const irreversible: IrreversibleEntry[] = [];

  // Reverse order: the last thing done is the first thing undone.
  for (let index = steps.length - 1; index >= 0; index--) {
    if (!journal.completed.has(index)) continue;

    const step = steps[index]!;
    const label = step.label ?? step.type;

    // Steps that never touched the outside world have nothing to reverse, and
    // saying "cannot undo a verification" would be noise, not honesty.
    if (!hasSideEffect(step)) continue;

    if (!step.compensate) {
      irreversible.push({
        stepIndex: index,
        stepId: step.id,
        stepLabel: label,
        reason: `no compensation is defined for this ${step.type} step`,
      });
      continue;
    }

    const verdict = classifyCompensation(step.compensate);
    entries.push({
      stepIndex: index,
      stepId: step.id,
      stepLabel: label,
      compensation: step.compensate,
      requiresApproval: verdict.sensitive,
      ...(verdict.reason ? { approvalReason: verdict.reason } : {}),
    });
  }

  return { entries, irreversible, complete: irreversible.length === 0 };
}

/**
 * Does this step change anything outside the browser's own view?
 *
 * Navigations, waits, reads and assertions do not, so they need no reversal.
 * Everything else does — including `fill`, because a filled field is state the
 * operator may want cleared even though it is not, by itself, a submission.
 */
export function hasSideEffect(step: WorkflowStep): boolean {
  switch (step.type) {
    case "click":
    case "fill":
    case "select":
    case "apiCall":
    case "sendEmail":
      return true;
    case "navigate":
    case "waitFor":
    case "extract":
    case "verify":
    case "approval":
      return false;
    default: {
      // Fail closed: an unclassified step type is assumed to have done
      // something, so it shows up as irreversible rather than silently ignored.
      const _never: never = step;
      void _never;
      return true;
    }
  }
}

/**
 * Decide whether a reversal needs human approval before it runs.
 *
 * Deliberately a **stricter** bar than the forward classifier, in one specific
 * way: any `click` inside a compensation gates, whatever its label says.
 *
 * The forward word list is tuned to avoid false positives — it matches
 * "cancel subscription" rather than bare "cancel", because gating every
 * "Cancel" button on every dismiss dialog would make the gate noise. That
 * trade-off is right on the way forward, where the click is one step among
 * many the human already reviewed. It is wrong on the way back: a compensation
 * only exists because a real side effect happened, every click in one is
 * reaching into a system to walk something back, and the operator asking for
 * an undo is exactly the person who should see it first.
 *
 * So: clicks always gate; everything else defers to `classifyStep`, which still
 * catches sensitive field entry and connector calls. Fail closed, and let the
 * cheap false positive land on the rare path rather than the common one.
 */
export function classifyCompensation(compensation: Compensation): {
  sensitive: boolean;
  reason?: string;
} {
  for (const action of compensation.actions) {
    if (action.type === "click") {
      const label = action.selector.name ?? action.selector.text ?? action.description;
      return {
        sensitive: true,
        reason: label
          ? `Reversal clicks "${label}"`
          : "Reversal clicks a control",
      };
    }

    const verdict = classifyStep(actionAsStep(action));
    if (verdict.sensitive) {
      return { sensitive: true, reason: verdict.reason };
    }
  }
  return { sensitive: false };
}

/** Lift a compensation action into a `WorkflowStep` so the classifier can read it. */
export function actionAsStep(action: CompensationAction, id = "compensation"): WorkflowStep {
  switch (action.type) {
    case "navigate":
      return { id, type: "navigate", url: action.url };
    case "click":
      return {
        id,
        type: "click",
        selector: action.selector,
        ...(action.description ? { description: action.description } : {}),
      };
    case "fill":
      return { id, type: "fill", selector: action.selector, value: action.value, sensitive: false };
    case "select":
      return { id, type: "select", selector: action.selector, value: action.value };
    case "waitFor":
      return {
        id,
        type: "waitFor",
        ...(action.selector ? { selector: action.selector } : {}),
        ...(action.urlPattern ? { urlPattern: action.urlPattern } : {}),
        ...(action.ms ? { ms: action.ms } : {}),
      };
  }
}
