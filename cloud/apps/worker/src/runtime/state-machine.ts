import { classifyStep } from "@ghost/core/classifier";
import type { WorkflowStep } from "@ghost/core/schema/step";

/**
 * Pure run-state machine. Given the workflow steps, the current cursor, and the
 * set of step indices that already have an APPROVED approval, decide the single
 * next action. Kept free of Playwright and the database so it is exhaustively
 * unit-testable — the execution engine (`jobs/runWorkflow.ts`) is just the
 * imperative shell around this.
 *
 * This is the enforcement point for MVP pillar 4: a sensitive step yields a
 * `gate` until a human approval for its index exists. The engine cannot skip it.
 */
export type NextAction =
  | { kind: "execute"; index: number; step: WorkflowStep }
  | { kind: "gate"; index: number; step: WorkflowStep; reason: string }
  | { kind: "done" };

export function planNextAction(
  steps: WorkflowStep[],
  cursor: number,
  approvedIndices: ReadonlySet<number>,
): NextAction {
  if (cursor >= steps.length) return { kind: "done" };

  const step = steps[cursor]!;
  const verdict = classifyStep(step);

  if (verdict.sensitive && !approvedIndices.has(cursor)) {
    return {
      kind: "gate",
      index: cursor,
      step,
      reason: verdict.reason ?? "Requires approval",
    };
  }

  return { kind: "execute", index: cursor, step };
}

/**
 * Whether an executed step type performs a real action. `approval` steps are
 * pure gates — once approved there is nothing to do at the browser level, so the
 * engine records them as succeeded without touching the page.
 */
export function isNoopStep(step: WorkflowStep): boolean {
  return step.type === "approval";
}
