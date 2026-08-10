import { z } from "zod";
import { EDITABLE_STEP_TYPES, workflowSteps } from "@ghost/core/schema/step";
import { checkPublicHttpUrl } from "@ghost/core/net/public-url";

/**
 * Validation for authored workflow definitions arriving over HTTP.
 *
 * The editor validates as you type, but that is a convenience for the author.
 * This is the authority: steps decide what Ghost does to a customer's system,
 * so an array that reaches `WorkflowVersion.steps` is one the worker will
 * faithfully try to execute.
 */

/**
 * Steps must parse *and* be restricted to types the executor implements.
 *
 * `workflowSteps` alone would accept `apiCall`/`sendEmail`, which parse and
 * gate but perform nothing — a run containing one sails past it and reports
 * SUCCEEDED having skipped the step. The editor does not offer them; this stops
 * a hand-rolled POST from introducing one anyway.
 *
 * Navigate URLs are also checked here (and again in the worker) so a private
 * or metadata host never lands in a version — see `@ghost/core/net/public-url`.
 */
const editableTypes = new Set<string>(EDITABLE_STEP_TYPES);

export const authoredSteps = workflowSteps
  .min(1, "a workflow needs at least one step")
  .superRefine((steps, ctx) => {
    steps.forEach((step, i) => {
      if (!editableTypes.has(step.type)) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: [i, "type"],
          message: `step type "${step.type}" has no executor yet and cannot be saved`,
        });
      }
      if (step.type === "navigate") {
        const result = checkPublicHttpUrl(step.url);
        if (!result.ok) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: [i, "url"],
            message: result.reason,
          });
        }
      }
      if (step.compensate) {
        step.compensate.actions.forEach((action, j) => {
          if (action.type !== "navigate") return;
          const result = checkPublicHttpUrl(action.url);
          if (!result.ok) {
            ctx.addIssue({
              code: z.ZodIssueCode.custom,
              path: [i, "compensate", "actions", j, "url"],
              message: result.reason,
            });
          }
        });
      }
    });
  });

export const createWorkflowInput = z.object({
  name: z.string().trim().min(1, "name is required").max(200),
  description: z.string().trim().max(2_000).optional(),
  steps: authoredSteps,
  /** Set when this workflow is being published from a compiled Recording
   * proposal — see apps/web/src/app/api/workflows/route.ts. */
  recordingId: z.string().min(1).optional(),
});

export const publishVersionInput = z.object({
  steps: authoredSteps,
  note: z.string().trim().max(500).optional(),
});

/** Flatten a ZodError into `"steps.2.url: Invalid url"`-style lines. */
export function formatIssues(err: z.ZodError): string {
  return err.issues.map((i) => `${i.path.join(".") || "(root)"}: ${i.message}`).join("; ");
}
