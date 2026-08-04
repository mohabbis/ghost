import { prisma } from "@/lib/db";
import { finalizeIfTerminal } from "@/lib/recording-compiler";

/**
 * Recording detail view, shared between `GET /api/recordings/[id]` (a single
 * fetch) and `GET /api/recordings/[id]/stream` (an SSE poll loop) — same
 * reasoning as `run-view.ts`.
 *
 * While `compileStatus` is `RUNNING` this also drives one compiler poll via
 * `finalizeIfTerminal`, so callers never have to talk to the compiler
 * themselves. A transient compiler error is swallowed rather than thrown: the
 * recording stays `RUNNING` for one more tick and the next poll tries again,
 * treating a blip as "wait and retry" rather than a hard failure.
 *
 * That swallow also covers the compiler being absent entirely — a recording
 * left `RUNNING` by a deployment that has since removed its compiler simply
 * stops advancing, rather than turning every detail fetch into a 500.
 */
export interface RecordingView {
  id: string;
  status: string;
  compileStatus: string;
  rawTraceFilename: string | null;
  compiledSteps: unknown;
  compileNotes: string | null;
  compileError: string | null;
  workflowId: string | null;
}

export async function buildRecordingView(orgId: string, id: string): Promise<RecordingView | null> {
  const existing = await prisma.recording.findFirst({
    where: { id, orgId },
    select: { id: true, compileStatus: true },
  });
  if (!existing) return null;

  if (existing.compileStatus === "RUNNING") {
    await finalizeIfTerminal(id).catch(() => undefined);
  }

  return prisma.recording.findFirst({
    where: { id, orgId },
    select: {
      id: true,
      status: true,
      compileStatus: true,
      rawTraceFilename: true,
      compiledSteps: true,
      compileNotes: true,
      compileError: true,
      workflowId: true,
    },
  });
}
