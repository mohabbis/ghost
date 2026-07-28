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

const base = {
  /** Stable id for referencing a step across edits and run traces. */
  id: z.string().min(1),
  /** Human-readable label shown in the editor and run timeline. */
  label: z.string().optional(),
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

/** Step type strings that mutate external state (used by the classifier). */
export const MUTATING_STEP_TYPES = new Set<WorkflowStep["type"]>([
  "click",
  "select",
  "apiCall",
  "sendEmail",
]);

/** Parse + validate a raw steps array, throwing on malformed input. */
export function parseWorkflowSteps(input: unknown): WorkflowSteps {
  return workflowSteps.parse(input);
}
