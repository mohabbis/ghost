import { isAtMostOnce } from "@ghost/core/classifier";
import { replaySafety } from "@ghost/core/classifier/replay";
import type { WorkflowStep } from "@ghost/core/schema/step";

/**
 * Plan how to rebuild browser state for a resumed run — without re-running any
 * side effect.
 *
 * Ghost closes Chromium when it halts at an approval gate, so a resumed run
 * would otherwise start on `about:blank` and fail to find the gated control.
 * The previous approach replayed every prior step through `applyStep`, clicks
 * included; with two gates in one workflow, resuming at the second re-clicked
 * the first one's already-approved button.
 *
 * The replacement restores the captured `storageState`, returns to the recorded
 * URL, and re-applies only steps that are both **restorative** and were
 * **recorded on that same URL**.
 *
 * The URL scoping is not incidental. A `fill` recorded on page A, re-applied
 * while the browser sits on page B, either throws or — far worse — resolves
 * `{role: "textbox", name: "Amount"}` against a *different* form on page B and
 * silently types into it. Type-based filtering alone would trade a known
 * double-submit bug for an unknown wrong-target one.
 *
 * Pure: no Playwright, no database.
 */

export interface ExecutedRecord {
  index: number;
  /** `page.url()` at the moment the step completed, if it was recorded. */
  url: string | null;
}

export type RestorePlan =
  /** Nothing has executed yet — start clean. */
  | { kind: "cold" }
  /** Navigate to `url`, then re-apply `reapply` (indices into `steps`). */
  | { kind: "restore"; url: string; reapply: number[] }
  /** State cannot be provably rebuilt. Stop and ask a human. */
  | { kind: "unsafe"; reason: string; blockingIndex: number | null };

/**
 * Did this step change anything beyond the browser's own view?
 *
 * Distinct from `isAtMostOnce`, which asks "must a human approve this?". An
 * `approval` step answers yes to that (it *is* a gate) while doing nothing at
 * all to the page. Restoration cares about the second question, not the first.
 */
function touchesOutsideWorld(step: WorkflowStep): boolean {
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
      // Fail closed: an unclassified step type is assumed to have acted.
      const _never: never = step;
      void _never;
      return true;
    }
  }
}

export function planRestore(
  steps: readonly WorkflowStep[],
  executed: readonly ExecutedRecord[],
  lastUrl: string | null,
): RestorePlan {
  if (executed.length === 0) return { kind: "cold" };

  if (!lastUrl) {
    return {
      kind: "unsafe",
      reason:
        "no captured page URL for this run — its browser state cannot be rebuilt " +
        "without replaying steps, which may repeat an action that already happened",
      blockingIndex: null,
    };
  }

  // A previously-executed at-most-once step whose effect landed on a page we
  // are no longer on: we can neither reproduce that state nor confirm it, and
  // we will not re-run the action to find out.
  for (const record of executed) {
    const step = steps[record.index];
    if (!step || !isAtMostOnce(step)) continue;
    // ...but only if it actually did something. An `approval` step is
    // at-most-once by classification (it is a gate, so it is "sensitive") yet
    // it never touches the page, and the engine records it with `url: null`.
    // Treating that null as "a different page" made every workflow with an
    // explicit approval gate followed by a browser step refuse to resume.
    if (!touchesOutsideWorld(step)) continue;
    if (record.url !== lastUrl) {
      return {
        kind: "unsafe",
        reason:
          `step ${record.index} performed an action that must not be repeated, on a ` +
          `different page (${record.url ?? "unknown"}) than the one being restored ` +
          `(${lastUrl})`,
        blockingIndex: record.index,
      };
    }
  }

  const reapply = executed
    .filter((record) => {
      const step = steps[record.index];
      if (!step) return false;
      if (replaySafety(step) !== "restorative") return false;
      // Only re-apply what was recorded on the page we are restoring to.
      if (record.url !== lastUrl) return false;
      // A `navigate` is subsumed by going to `lastUrl` directly.
      return step.type !== "navigate";
    })
    .map((record) => record.index)
    .sort((a, b) => a - b);

  return { kind: "restore", url: lastUrl, reapply };
}
