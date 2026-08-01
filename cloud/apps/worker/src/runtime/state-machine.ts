import { classifyStep, isAtMostOnce } from "@ghost/core/classifier";
import type { WorkflowStep } from "@ghost/core/schema/step";
import type { RunJournal } from "@ghost/core/journal";

/**
 * Pure run-state machine. Given the workflow steps, what the journal says has
 * already happened, and the recorded approvals, decide the single next action.
 * Kept free of Playwright and the database so it is exhaustively unit-testable —
 * the execution engine (`jobs/runWorkflow.ts`) is just the imperative shell.
 *
 * Two invariants live here.
 *
 * **Pillar 4 (approval):** a sensitive step yields a `gate` until a human
 * approval for its index exists. The engine cannot skip it.
 *
 * **Durable execution:** position is derived by folding the journal, not read
 * from a stored cursor. A step with a recorded terminal success is skipped, so
 * re-executing one is not a bug to remember to avoid — it is a state this
 * function cannot return. This is what makes a crash mid-run safe: previously
 * the cursor was a local variable persisted only at gates, so a redelivered job
 * re-ran completed steps, and since approvals are re-read from the database an
 * already-approved payment re-planned as `execute` rather than `gate`.
 */

export type NextAction =
  | { kind: "execute"; index: number; step: WorkflowStep }
  | { kind: "gate"; index: number; step: WorkflowStep; reason: string }
  /**
   * An at-most-once step was interrupted mid-flight. Never auto-resume: we
   * cannot tell whether the effect landed, and neither re-running nor skipping
   * is the engine's decision to make.
   */
  | { kind: "indeterminate"; index: number; step: WorkflowStep; reason: string }
  /**
   * `recoverable` separates a step that failed at runtime — which a human can
   * retry or skip through the incident flow — from a decision or an authoring
   * error that retrying cannot change. A rejected approval and an unresolvable
   * expression are terminal; a step that errored is not.
   */
  | { kind: "failed"; index: number; reason: string; recoverable: boolean }
  | { kind: "done" };

export interface ApprovalState {
  approved: ReadonlySet<number>;
  rejected: ReadonlySet<number>;
}

/**
 * Resolves a step's `{{ }}` references against values captured earlier in the
 * run. Applied to the candidate step *before* it is classified, so the gate
 * decision and the approval preview describe the action that will actually
 * run — approving `Pay {{ steps.total.amount }}` and resolving it afterwards would
 * make the gate theater. Must be pure; throwing means an unresolvable
 * reference, which is an authoring error and fails the run.
 */
export type StepResolver = (step: WorkflowStep, index: number) => WorkflowStep;

const IDENTITY: StepResolver = (step) => step;

export function planNextAction(
  steps: readonly WorkflowStep[],
  journal: RunJournal,
  approvals: ApprovalState,
  resolve: StepResolver = IDENTITY,
): NextAction {
  // Linear scan rather than a cursor, deliberately. Workflows are tens of
  // steps, and the O(n) walk is what removes the mutable position that caused
  // completed steps to re-execute. Do not "optimize" this back into a cursor.
  for (let index = 0; index < steps.length; index++) {
    const raw = steps[index]!;

    // Durable execution: already done, in this run or a previous attempt.
    if (journal.completed.has(index)) continue;

    if (journal.failed.has(index)) {
      // Recoverable: the incident flow (retry / skip) exists for exactly this.
      return {
        kind: "failed",
        index,
        reason: `step ${index} previously failed`,
        recoverable: true,
      };
    }

    if (approvals.rejected.has(index)) {
      // A human decided. Retrying is not the engine's to offer.
      return {
        kind: "failed",
        index,
        reason: `approval rejected at step ${index}`,
        recoverable: false,
      };
    }

    let step: WorkflowStep;
    try {
      step = resolve(raw, index);
    } catch (err) {
      // An unresolvable reference is an authoring error: the workflow
      // definition is wrong, and running it again produces the same error.
      return {
        kind: "failed",
        index,
        reason: `step ${index}: ${err instanceof Error ? err.message : String(err)}`,
        recoverable: false,
      };
    }

    // The crash window. For a step whose effect must not happen twice, stop.
    if (journal.inFlight.has(index) && isAtMostOnce(step)) {
      return {
        kind: "indeterminate",
        index,
        step,
        reason:
          `step ${index} started but never recorded an outcome, and its effect ` +
          `cannot safely be repeated`,
      };
    }

    // A benign step interrupted mid-flight is safe to re-attempt, and the
    // retry is visible in the journal and the run timeline.

    const verdict = classifyStep(step);
    if (verdict.sensitive && !approvals.approved.has(index)) {
      return {
        kind: "gate",
        index,
        step,
        reason: verdict.reason ?? "Requires approval",
      };
    }

    return { kind: "execute", index, step };
  }

  return { kind: "done" };
}

/**
 * Whether an executed step type performs a real action. `approval` steps are
 * pure gates — once approved there is nothing to do at the browser level, so the
 * engine records them as succeeded without touching the page.
 */
export function isNoopStep(step: WorkflowStep): boolean {
  return step.type === "approval";
}
