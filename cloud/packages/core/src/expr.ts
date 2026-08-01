import type { WorkflowStep } from "./schema/step.js";

/**
 * The expression resolver — deliberately not a language.
 *
 * n8n gives workflows JavaScript expressions. Ghost cannot: engineering rule 1
 * is *"AI may propose; deterministic code executes only approved plans."* An
 * approved plan containing arbitrary code is not a plan — it is a program that
 * can compute a different action at runtime than the one the human reviewed.
 *
 * So this resolves exactly one form and nothing else:
 *
 *   {{ steps.<stepId>.<name> }}   a value captured by an earlier `extract`
 *
 * A `{{ vars.<name> }}` form is deliberately absent until runs can actually
 * carry inputs. Advertising a syntax that fails every time it is used is worse
 * than not having it; run inputs arrive with the typed step editor.
 *
 * No `eval`, no arithmetic, no method calls, no property walks. `{{ 1 + 1 }}`
 * is not an expression, it is an error. An unresolved reference is a hard
 * failure, never a silent empty string — quietly substituting "" into a payment
 * amount is precisely the class of bug this product exists to prevent.
 *
 * Two rules travel with this and are enforced by the caller, not here:
 *   1. Interpolation happens BEFORE classification and BEFORE the approval
 *      preview, so a human approves the resolved action.
 *   2. A resolved value flowing into a sensitive field re-triggers the
 *      sensitivity classifier.
 */

export class ExpressionError extends Error {
  constructor(
    message: string,
    readonly reference: string,
  ) {
    super(message);
    this.name = "ExpressionError";
  }
}

export interface ExpressionScope {
  /** Outputs by step id: `steps[stepId][name]`. */
  steps: Readonly<Record<string, Readonly<Record<string, string>>>>;
}

export const EMPTY_SCOPE: ExpressionScope = { steps: {} };

/**
 * One `{{ ... }}` placeholder. The inner grammar is fully anchored — anything
 * that is not the accepted shape fails to match and is reported as
 * an unsupported expression rather than being evaluated.
 */
const PLACEHOLDER = /\{\{([^}]*)\}\}/g;
const STEP_REF = /^steps\.([A-Za-z0-9_-]+)\.([A-Za-z0-9_-]+)$/;

/** True when the string contains at least one placeholder. */
export function hasExpression(input: string): boolean {
  PLACEHOLDER.lastIndex = 0;
  return PLACEHOLDER.test(input);
}

/** Resolve every placeholder in `input`. Throws on anything unresolvable. */
export function interpolate(input: string, scope: ExpressionScope): string {
  return input.replace(PLACEHOLDER, (_match, rawRef: string) => {
    const ref = rawRef.trim();

    const stepMatch = STEP_REF.exec(ref);
    if (stepMatch) {
      const [, stepId, name] = stepMatch as unknown as [string, string, string];
      const value = scope.steps[stepId]?.[name];
      if (value === undefined) {
        throw new ExpressionError(
          `no value named "${name}" was captured by step "${stepId}"`,
          ref,
        );
      }
      return value;
    }

    throw new ExpressionError(
      `unsupported expression "${ref}" — only {{ steps.<id>.<name> }} is allowed`,
      ref,
    );
  });
}

/**
 * Return a copy of `step` with its interpolatable fields resolved.
 *
 * Only fields whose value is *data* are resolved. Selectors are deliberately
 * excluded: letting an extracted value choose which element to click would make
 * the reviewed plan and the executed action come apart, which is the same
 * failure mode as allowing arbitrary code.
 */
export function resolveStep(step: WorkflowStep, scope: ExpressionScope): WorkflowStep {
  switch (step.type) {
    case "navigate":
      return { ...step, url: interpolate(step.url, scope) };
    case "fill":
      return { ...step, value: interpolate(step.value, scope) };
    case "select":
      return { ...step, value: interpolate(step.value, scope) };
    case "sendEmail":
      return {
        ...step,
        to: step.to.map((addr) => interpolate(addr, scope)),
        subject: interpolate(step.subject, scope),
        body: interpolate(step.body, scope),
      };
    case "apiCall":
      return {
        ...step,
        input: step.input
          ? Object.fromEntries(
              Object.entries(step.input).map(([k, v]) => [
                k,
                typeof v === "string" ? interpolate(v, scope) : v,
              ]),
            )
          : step.input,
      };
    // Nothing data-bearing to resolve.
    case "click":
    case "waitFor":
    case "extract":
    case "verify":
    case "approval":
      return step;
    default: {
      const _never: never = step;
      void _never;
      return step;
    }
  }
}
