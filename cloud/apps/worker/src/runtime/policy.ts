import { isAtMostOnce } from "@ghost/core/classifier";
import type { RetryPolicy, WorkflowStep } from "@ghost/core/schema/step";

/**
 * Runtime resolution of a step's timeout and retry policy.
 *
 * Lives in the worker rather than the Zod schema because the clamp depends on
 * the sensitivity classifier, and `classifyStep` imports its types from
 * `schema/step.ts` — enforcing it there would be an import cycle. It also
 * belongs at execution time on principle: a workflow definition authored (or
 * edited) with `retry.maxAttempts: 5` on a payment step must not be able to
 * talk the engine into retrying it, whatever the stored JSON says.
 */

const DEFAULT_TIMEOUT_MS = 30_000;
const NO_RETRY: RetryPolicy = { maxAttempts: 1, backoffMs: 0, factor: 2 };

export function effectiveTimeout(step: WorkflowStep): number {
  if (step.timeoutMs) return step.timeoutMs;
  const fromEnv = Number(process.env.GHOST_STEP_TIMEOUT_MS);
  return Number.isFinite(fromEnv) && fromEnv > 0 ? fromEnv : DEFAULT_TIMEOUT_MS;
}

/**
 * The retry policy the engine will actually honour.
 *
 * Forced to a single attempt for any at-most-once step. Retrying a "Submit
 * order" click that timed out is a double-submit wearing a different hat — the
 * request may well have reached the server.
 */
export function effectiveRetry(step: WorkflowStep): RetryPolicy {
  if (isAtMostOnce(step)) return NO_RETRY;
  return step.retry ?? NO_RETRY;
}

/** Backoff before attempt `attempt` (1-based: attempt 2 is the first retry). */
export function backoffFor(policy: RetryPolicy, attempt: number): number {
  if (attempt <= 1) return 0;
  return Math.round(policy.backoffMs * Math.pow(policy.factor, attempt - 2));
}

/** Reject a policy the runtime would silently ignore, for the future step editor. */
export function retryPolicyIssues(step: WorkflowStep): string[] {
  const issues: string[] = [];
  if (step.retry && step.retry.maxAttempts > 1 && isAtMostOnce(step)) {
    issues.push(
      `step "${step.id}" requires approval, so it cannot be retried automatically; ` +
        `maxAttempts is forced to 1`,
    );
  }
  return issues;
}
