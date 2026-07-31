import { isAtMostOnce } from "@ghost/core/classifier";
import type { WorkflowStep } from "@ghost/core/schema/step";

/**
 * Classify a thrown execution error so the shell knows whether re-attempting is
 * safe. Pure, table-driven, fail-closed.
 *
 * The distinction that matters: a timeout on a *benign* step means the page was
 * slow, and re-attempting costs nothing. A timeout on an at-most-once step means
 * the click may already have reached the server — re-attempting could take the
 * payment twice. Same exception, opposite handling.
 */

export type ErrorClass =
  /** Transient. Safe to re-attempt. */
  | "retryable"
  /** Deterministic — usually an authoring bug. Retrying cannot help. */
  | "terminal"
  /** The effect may or may not have landed. A human decides. */
  | "indeterminate";

const TRANSPORT_PATTERNS = [
  "net::err_",
  "ns_error_",
  "econnreset",
  "econnrefused",
  "etimedout",
  "socket hang up",
];

const CRASHED_PAGE_PATTERNS = [
  "target closed",
  "target page, context or browser has been closed",
  "browser has been closed",
  "execution context was destroyed",
];

/** Authoring mistakes: the workflow definition is wrong, not the environment. */
const AUTHORING_PATTERNS = [
  "strict mode violation",
  "selector has no resolvable field",
  "is not a valid selector",
  "unsupported expression",
];

export function classifyError(err: unknown, step: WorkflowStep): ErrorClass {
  const message = (err instanceof Error ? err.message : String(err)).toLowerCase();
  const atMostOnce = isAtMostOnce(step);

  // Authoring bugs are checked first: they are deterministic regardless of the
  // step's risk class, and retrying one can only hit the wrong element again.
  if (AUTHORING_PATTERNS.some((p) => message.includes(p))) return "terminal";

  const isTimeout =
    err instanceof Error &&
    (err.name === "TimeoutError" ||
      message.includes("timeout") ||
      message.includes("timed out"));

  if (isTimeout) return atMostOnce ? "indeterminate" : "retryable";

  if (TRANSPORT_PATTERNS.some((p) => message.includes(p))) {
    // A transport error before an at-most-once action is usually safe, but we
    // cannot tell whether the request reached the server. Fail closed.
    return atMostOnce ? "indeterminate" : "retryable";
  }

  if (CRASHED_PAGE_PATTERNS.some((p) => message.includes(p))) {
    return atMostOnce ? "indeterminate" : "retryable";
  }

  // Anything unrecognized: do not guess.
  return "terminal";
}
