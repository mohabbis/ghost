import { appendAuditEvent } from "@ghost/core/audit-log";
import type { WorkflowSteps } from "@ghost/core/schema/step";
import { Prisma, type RecordingCompileStatus } from "@ghost/core/db";
import { prisma } from "@/lib/db";
import { requireWorkflowCompiler, type CompileJob } from "@/lib/compiler";

/**
 * Orchestrates recording compilation: owns the database, the audit trail and
 * the status machine, and delegates every judgement about *how* a trace
 * becomes steps to a `WorkflowCompiler`.
 *
 * Nothing in this file knows which compiler is configured, or that one might
 * not be. It calls `requireWorkflowCompiler()`, which throws
 * `CompilerNotConfiguredError` when there is none; routes render that as 503.
 *
 * The compiler's output is a *proposal*. It lands in `Recording.compiledSteps`
 * and is never executed. A human reviews it in the same `WorkflowEditor` used
 * for hand-authored workflows and explicitly publishes it through the existing
 * `POST /api/workflows` path, which revalidates every step. Nothing here
 * bypasses that trust pipeline.
 */

export interface RecordingForCompile {
  id: string;
  orgId: string;
  rawTraceKey: string | null;
  rawTraceFilename: string | null;
}

/** Starts a compile and returns the handle to persist. Does not wait for
 * completion — a compile outlives the request that starts it. */
export async function startCompile(recording: RecordingForCompile): Promise<CompileJob> {
  if (!recording.rawTraceKey) {
    throw new Error("recording has no uploaded trace to compile");
  }
  return requireWorkflowCompiler().start({
    recordingId: recording.id,
    orgId: recording.orgId,
    traceKey: recording.rawTraceKey,
    filename: recording.rawTraceFilename ?? "recording-trace",
  });
}

/** Resumes a compile that stopped short of a result. */
export async function continueCompile(job: CompileJob): Promise<CompileJob> {
  return requireWorkflowCompiler().resume(job);
}

export async function cancelCompile(job: CompileJob): Promise<void> {
  await requireWorkflowCompiler().cancel(job);
}

interface FinalizeResult {
  /** `null` when the recording does not exist (already deleted, or a bad id). */
  compileStatus: RecordingCompileStatus | null;
  terminal: boolean;
}

/**
 * Polls one recording's compile and, the first time it observes a terminal
 * state, persists the result. Idempotent to call repeatedly while `RUNNING` —
 * a no-op once the recording's `compileStatus` has already left `RUNNING`.
 *
 * Looks the recording up by id alone and reads `orgId` off the row to
 * attribute the audit event. That is safe only because every caller has
 * already done an org-scoped existence check; it is a caller contract, not
 * something enforced here.
 */
export async function finalizeIfTerminal(recordingId: string): Promise<FinalizeResult> {
  const recording = await prisma.recording.findUnique({
    where: { id: recordingId },
    select: {
      id: true,
      orgId: true,
      compileStatus: true,
      compileJobId: true,
      compileSessionId: true,
    },
  });
  if (!recording) return { compileStatus: null, terminal: true };
  if (
    recording.compileStatus !== "RUNNING" ||
    !recording.compileSessionId ||
    !recording.compileJobId
  ) {
    return { compileStatus: recording.compileStatus, terminal: true };
  }

  const progress = await requireWorkflowCompiler().poll({
    jobId: recording.compileJobId,
    sessionId: recording.compileSessionId,
  });

  if (progress.state === "running") {
    return { compileStatus: "RUNNING", terminal: false };
  }

  if (progress.state === "failed") {
    const compileStatus = await persistOutcome(recording.id, recording.orgId, {
      compileStatus: "FAILED",
      compileNotes: progress.notes ?? null,
      compileError: progress.reason,
    });
    return { compileStatus, terminal: true };
  }

  const compileStatus = await persistOutcome(recording.id, recording.orgId, {
    compileStatus: "READY",
    compiledSteps: progress.steps,
    compileNotes: progress.notes,
  });
  return { compileStatus, terminal: true };
}

async function persistOutcome(
  recordingId: string,
  orgId: string,
  outcome: {
    compileStatus: "READY" | "FAILED";
    compiledSteps?: WorkflowSteps;
    compileNotes?: string | null;
    compileError?: string;
  },
): Promise<RecordingCompileStatus> {
  await prisma.$transaction(async (tx) => {
    await tx.recording.update({
      where: { id: recordingId },
      data: {
        compileStatus: outcome.compileStatus,
        compiledSteps:
          outcome.compiledSteps !== undefined
            ? (outcome.compiledSteps as unknown as Prisma.InputJsonValue)
            : undefined,
        compileNotes: "compileNotes" in outcome ? outcome.compileNotes : undefined,
        compileError: outcome.compileError ?? null,
      },
    });
    await appendAuditEvent(
      orgId,
      null,
      {
        action:
          outcome.compileStatus === "READY" ? "recording.compile_ready" : "recording.compile_failed",
        entityType: "Recording",
        entityId: recordingId,
        metadata:
          outcome.compileStatus === "READY"
            ? { stepCount: outcome.compiledSteps?.length ?? 0 }
            : { error: outcome.compileError },
      },
      tx,
    );
  });
  return outcome.compileStatus;
}
