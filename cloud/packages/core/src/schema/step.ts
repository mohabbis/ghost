import { z } from "zod";

/**
 * The workflow step schema.
 *
 * A workflow is an ordered list of typed steps. Steps target elements by
 * *semantic* selectors (role+name, test id, text, or CSS) — never raw pixel
 * coordinates. This is the editable representation a user reviews, and the exact
 * thing the worker executes.
 *
 * Stored as JSON on `WorkflowVersion.steps` and validated with `workflowSteps`.
 */

/** A robust element locator. Resolution prefers the most stable available. */
export const selectorSchema = z.object({
  /** ARIA role, e.g. "button", "textbox", "link". */
  role: z.string().optional(),
  /** Accessible name / visible text used with `role`. */
  name: z.string().optional(),
  /** `data-testid` or equivalent. */
  testId: z.string().optional(),
  /** Visible text content. */
  text: z.string().optional(),
  /** Raw CSS selector — last resort. */
  css: z.string().optional(),
});
export type Selector = z.infer<typeof selectorSchema>;

/**
 * Per-step retry policy, in the spirit of a Temporal activity's retry options.
 *
 * Authoring a retry on a sensitive step is a mistake the runtime refuses to
 * honour — see `effectiveRetry` in apps/worker/src/runtime/policy.ts, which
 * clamps `maxAttempts` to 1 for anything the classifier calls sensitive.
 * Re-clicking "Pay" because the first click timed out is a double payment, not
 * a retry.
 */
export const retryPolicySchema = z.object({
  maxAttempts: z.number().int().min(1).max(5).default(1),
  backoffMs: z.number().int().min(0).max(60_000).default(1_000),
  /** Multiplier applied to the backoff after each failed attempt. */
  factor: z.number().min(1).max(10).default(2),
});
export type RetryPolicy = z.infer<typeof retryPolicySchema>;

const base = {
  /** Stable id for referencing a step across edits and run traces. */
  id: z.string().min(1),
  /** Human-readable label shown in the editor and run timeline. */
  label: z.string().optional(),
  /**
   * Timeout for this step's browser **action** — the click, fill, navigation
   * or wait itself. Falls back to GHOST_STEP_TIMEOUT_MS (default 30s).
   *
   * Deliberately *not* a wall-clock budget for the whole step: retry backoff,
   * verification and its retries, the screenshot, and the artifact upload all
   * run outside it, and a `waitFor` with an explicit `ms` sleeps for that long
   * regardless. A step with `timeoutMs: 1000` can therefore still occupy a
   * worker for considerably longer. Named for what it actually bounds rather
   * than what would be nicer to promise.
   *
   * Optional, so definitions written before this field existed still parse.
   */
  timeoutMs: z.number().int().positive().max(600_000).optional(),
  retry: retryPolicySchema.optional(),
};

/** Optional post-step verification assertion. */
export const verificationSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("url"), expected: z.string() }),
  z.object({ kind: z.literal("selectorVisible"), selector: selectorSchema }),
  z.object({ kind: z.literal("textPresent"), expected: z.string() }),
]);
export type Verification = z.infer<typeof verificationSchema>;

export const navigateStep = z.object({
  ...base,
  type: z.literal("navigate"),
  url: z.string().url(),
  verify: verificationSchema.optional(),
});

export const clickStep = z.object({
  ...base,
  type: z.literal("click"),
  selector: selectorSchema,
  description: z.string().optional(),
  verify: verificationSchema.optional(),
});

export const fillStep = z.object({
  ...base,
  type: z.literal("fill"),
  selector: selectorSchema,
  value: z.string(),
  /** Marks the field as holding secret/PII input (password, OTP, card). */
  sensitive: z.boolean().default(false),
  verify: verificationSchema.optional(),
});

export const selectStep = z.object({
  ...base,
  type: z.literal("select"),
  selector: selectorSchema,
  value: z.string(),
});

export const waitForStep = z.object({
  ...base,
  type: z.literal("waitFor"),
  selector: selectorSchema.optional(),
  urlPattern: z.string().optional(),
  ms: z.number().int().positive().optional(),
});

export const extractStep = z.object({
  ...base,
  type: z.literal("extract"),
  selector: selectorSchema,
  /** Name the extracted value is stored under for later steps. */
  name: z.string().min(1),
});

export const verifyStep = z.object({
  ...base,
  type: z.literal("verify"),
  assertion: verificationSchema,
});

/** Explicit human-approval gate authored into the workflow. */
export const approvalStep = z.object({
  ...base,
  type: z.literal("approval"),
  reason: z.string().min(1),
});

// --- Post-MVP step types (reserved; no executor yet) ---------------------

export const apiCallStep = z.object({
  ...base,
  type: z.literal("apiCall"),
  connectorId: z.string(),
  operation: z.string(),
  input: z.record(z.string(), z.unknown()).optional(),
});

export const sendEmailStep = z.object({
  ...base,
  type: z.literal("sendEmail"),
  connectorId: z.string(),
  to: z.array(z.string()),
  subject: z.string(),
  body: z.string(),
});

export const workflowStep = z.discriminatedUnion("type", [
  navigateStep,
  clickStep,
  fillStep,
  selectStep,
  waitForStep,
  extractStep,
  verifyStep,
  approvalStep,
  apiCallStep,
  sendEmailStep,
]);
export type WorkflowStep = z.infer<typeof workflowStep>;

export const workflowSteps = z.array(workflowStep);
export type WorkflowSteps = z.infer<typeof workflowSteps>;

// `MUTATING_STEP_TYPES` used to live here. It was dead code — exported, never
// imported, and its comment claimed the classifier used it when the classifier
// has always used its own word lists. It also conflated "mutates the DOM" with
// "mutates the world" by including `select`, which is a client-side form change
// exactly like `fill`. Replay safety now lives in `classifier/replay.ts`, which
// draws that line deliberately; keeping a second, wrong copy of it here would
// be worse than having none.

/** Parse + validate a raw steps array, throwing on malformed input. */
export function parseWorkflowSteps(input: unknown): WorkflowSteps {
  return workflowSteps.parse(input);
}
