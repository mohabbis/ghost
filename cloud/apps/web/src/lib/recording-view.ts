import { prisma } from "@/lib/db";
import { finalizeIfTerminal } from "@/lib/recording-compiler";

/**
 * Recording detail view, shared between `GET /api/recordings/[id]` (a single
 * fetch) and `GET /api/recordings/[id]/stream` (an SSE poll loop) — same
 * reasoning as `run-view.ts`.
 *
 * While `compileStatus` is `RUNNING` this also drives one HarnessRouter
 * `getSession` poll via `finalizeIfTerminal`, so callers never need to poll
 * HarnessRouter themselves. A transient HarnessRouter error here is
 * swallowed rather than thrown: the recording just stays `RUNNING` for one
 * more tick and the next poll tries again, matching how the setup guide
 * treats a HarnessRouter blip as "wait and retry", not a hard failure.
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
