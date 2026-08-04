import type { WorkflowSteps } from "@ghost/core/schema/step";

/**
 * The boundary between Ghost and whatever turns a recording trace into
 * proposed steps.
 *
 * Compilation is an *authoring convenience*, not part of the runtime. Its
 * output is a proposal a human reviews in the editor and publishes through the
 * ordinary `POST /api/workflows` path, which revalidates every step. Nothing
 * here executes, and nothing downstream of a published workflow depends on a
 * compiler existing.
 *
 * That is why the interface is deliberately small and why `workflowCompiler()`
 * is allowed to return `null`: Ghost must boot, replay workflows, gate, verify
 * and audit with no compiler configured at all. Any implementation of this
 * interface is replaceable, and removing every implementation may only disable
 * compilation — see `docs/DEPLOY.md`.
 */
export interface WorkflowCompiler {
  /** Identifies the implementation in errors and audit metadata. */
  readonly name: string;

  /** Begins a compile. Returns as soon as the job is accepted; it does not
   * wait for completion, because a compile outlives the request that starts
   * it and must survive a redeploy of the web app. */
  start(trace: RecordingTrace): Promise<CompileJob>;

  /** Resumes a job that stopped short of producing a result. */
  resume(job: CompileJob): Promise<CompileJob>;

  /** Reports where a job has got to. Must be safe to call repeatedly. */
  poll(job: CompileJob): Promise<CompileProgress>;

  /** Best-effort stop. Implementations may return before the job has actually
   * reached a terminal state; the next `poll` is what observes it. */
  cancel(job: CompileJob): Promise<void>;
}

/** The input: one uploaded trace, addressed by artifact key. */
export interface RecordingTrace {
  recordingId: string;
  orgId: string;
  /** Artifact-store key of the raw uploaded trace. */
  traceKey: string;
  /** Original filename, which is the only hint at the trace's format. */
  filename: string;
}

/**
 * The handle persisted on `Recording` so a later request — possibly in a
 * different process — can poll, resume or cancel a compile it did not start.
 *
 * Both fields are opaque to Ghost. Do not parse them, and do not let an
 * implementation's vocabulary for them leak into the data model.
 */
export interface CompileJob {
  /** The unit of work. */
  jobId: string;
  /** The conversation or container the work happens in, if the implementation
   * distinguishes the two. Implementations that do not may repeat `jobId`. */
  sessionId: string;
}

export type CompileProgress =
  | { state: "running" }
  /** Steps are already validated against `@ghost/core/schema/step`. An
   * implementation that cannot produce valid steps reports `failed` rather
   * than handing back something for the caller to sort out. */
  | { state: "ready"; steps: WorkflowSteps; notes: string | null }
  | { state: "failed"; reason: string; notes?: string | null };

/** No compiler is configured. Callers should surface this as "unavailable"
 * (503), never as a server error: it is the expected production state. */
export class CompilerNotConfiguredError extends Error {
  constructor(message = "no workflow compiler is configured") {
    super(message);
    this.name = "CompilerNotConfiguredError";
  }
}

/**
 * The compiler was reachable and refused, or is misconfigured.
 *
 * `status` carries an upstream HTTP status where there is one, so a route can
 * distinguish "your fault" from "its fault" — an out-of-credit or bad-key
 * response is actionable by whoever runs the compiler, and turning it into a
 * bare 500 loses that.
 */
export class CompilerRequestError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "CompilerRequestError";
  }
}
