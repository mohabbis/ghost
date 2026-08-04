import { harnessRouterCompiler } from "./harness-router-compiler";
import {
  CompilerNotConfiguredError,
  CompilerRequestError,
  type WorkflowCompiler,
} from "./types";

export * from "./types";

/**
 * The configured compiler, or `null` when there is none.
 *
 * `null` is the expected production state. HarnessRouter is development
 * tooling — agent infrastructure used to build Ghost, not to run it — and
 * pointing it at a customer's recording means uploading that trace whole to a
 * third party the customer never contracted with. Production leaves
 * `HR_API_KEY` unset, which disables compilation and nothing else.
 *
 * Read the environment on every call rather than caching a decision at module
 * load: the value is then observable in tests without module-registry games,
 * and a misconfigured deploy fails at the request rather than at boot.
 */
export function workflowCompiler(): WorkflowCompiler | null {
  if (!process.env.HR_API_KEY) return null;
  return harnessRouterCompiler;
}

/** Same, for call sites that cannot proceed without one. Throws
 * `CompilerNotConfiguredError`, which routes render as 503. */
export function requireWorkflowCompiler(): WorkflowCompiler {
  const compiler = workflowCompiler();
  if (!compiler) throw new CompilerNotConfiguredError();
  return compiler;
}

/** Whether compilation is available, for UI that should not offer a button
 * leading to a 503. */
export function compilerConfigured(): boolean {
  return workflowCompiler() !== null;
}

/**
 * Renders a compiler failure as a response, or `null` if this is not one and
 * the caller should rethrow.
 *
 * Shared so the three compile routes cannot drift on status codes, and so
 * none of them has to import a vendor error type to decide what to say.
 *
 * - not configured → **503**, the expected production state, not a bug.
 * - upstream 4xx → passed through: "out of credits" or "bad key" is
 *   actionable by whoever runs the compiler, and flattening it to 500 loses
 *   that. Anything else → **502**, since the failure is not the caller's.
 */
export function compilerErrorResponse(err: unknown): Response | null {
  if (err instanceof CompilerNotConfiguredError) {
    return Response.json(
      { error: "recording compilation is not enabled on this deployment" },
      { status: 503 },
    );
  }
  if (err instanceof CompilerRequestError) {
    const status = err.status >= 400 && err.status < 500 ? err.status : 502;
    return Response.json({ error: err.message }, { status });
  }
  return null;
}
