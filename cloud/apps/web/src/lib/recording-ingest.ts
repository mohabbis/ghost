import { appendAuditEvent } from "@ghost/core/audit-log";
import { artifactStore } from "@ghost/core/storage/artifacts";
import { Prisma } from "@ghost/core/db";
import { compileTrace } from "@ghost/core/recording/compile";
import { parseRecordingTrace } from "@ghost/core/recording/trace";
import { prisma } from "@/lib/db";

/**
 * Storing an uploaded recording trace, shared by the browser upload form and
 * the extension's ingest endpoint so the two cannot drift on limits,
 * sanitisation, or what gets audited.
 *
 * The interesting part is what happens to a *structured* trace. A Ghost
 * recorder reads the accessible role and name off each element while it is
 * still on screen, so the trace already contains what the worker's resolution
 * chain wants. `compileTrace` then turns it into typed steps deterministically
 * — no model, no network, no configured compiler — and the recording lands
 * `READY` for review in a single request.
 *
 * That is what makes recording work in production, where there is deliberately
 * no compiler configured at all (see `docs/DEPLOY.md`). Anything that is *not*
 * a structured trace — a HAR, a Playwright zip — is stored as before and left
 * for whatever compiler exists, if any.
 */

/** Small event logs, not video. Well under Vercel's 100MB body limit. */
export const MAX_TRACE_BYTES = 25 * 1024 * 1024;

export function sanitizeFilename(name: string): string {
  const base = name.split(/[/\\]/).pop() || "recording-trace";
  return base.replace(/[^a-zA-Z0-9._-]/g, "_").slice(-200) || "recording-trace";
}

export interface IngestResult {
  id: string;
  /** READY when the trace compiled deterministically, NONE when it awaits a compiler. */
  compileStatus: "READY" | "NONE";
  stepCount: number;
  notes: string[];
}

export async function ingestTrace(args: {
  orgId: string;
  userId: string | null;
  filename: string;
  contentType: string;
  buffer: Buffer;
}): Promise<IngestResult> {
  const { orgId, userId, buffer } = args;
  const filename = sanitizeFilename(args.filename);

  const recording = await prisma.recording.create({
    data: { orgId, status: "STOPPED" },
  });

  try {
    const key = `recordings/${recording.id}/trace-${filename}`;
    await artifactStore().put(key, buffer, args.contentType || "application/octet-stream");

    const compiled = tryCompile(buffer);

    await prisma.$transaction(async (tx) => {
      await tx.recording.update({
        where: { id: recording.id },
        data: {
          rawTraceKey: key,
          rawTraceFilename: filename,
          ...(compiled
            ? {
                compileStatus: "READY" as const,
                compiledSteps: compiled.steps as unknown as Prisma.InputJsonValue,
                compileNotes: compiled.notes.length > 0 ? compiled.notes.join("\n\n") : null,
                compileError: null,
              }
            : {}),
        },
      });
      await appendAuditEvent(
        orgId,
        userId,
        {
          action: "recording.uploaded",
          entityType: "Recording",
          entityId: recording.id,
          metadata: { filename, bytes: buffer.byteLength },
        },
        tx,
      );
      if (compiled) {
        // Audited as a compile in its own right. A reviewer looking at how a
        // workflow came to exist should see that a compiler ran, even though
        // it ran inline and deterministically rather than as a queued task.
        await appendAuditEvent(
          orgId,
          userId,
          {
            action: "recording.compile_ready",
            entityType: "Recording",
            entityId: recording.id,
            metadata: { stepCount: compiled.steps.length, compiler: "deterministic" },
          },
          tx,
        );
      }
    });

    return {
      id: recording.id,
      compileStatus: compiled ? "READY" : "NONE",
      stepCount: compiled?.steps.length ?? 0,
      notes: compiled?.notes ?? [],
    };
  } catch (err) {
    // Storage failed after the row was created — don't leave an unusable
    // Recording with no trace behind for the org to trip over.
    await prisma.recording.delete({ where: { id: recording.id } }).catch(() => undefined);
    throw err;
  }
}

/** Returns compiled steps for a structured Ghost trace, or null for anything else. */
function tryCompile(buffer: Buffer): { steps: unknown[]; notes: string[] } | null {
  let json: unknown;
  try {
    json = JSON.parse(buffer.toString("utf8"));
  } catch {
    return null; // a zip, a HAR that isn't ours, or not JSON at all
  }
  const parsed = parseRecordingTrace(json);
  if (!parsed.ok) return null;

  const { steps, notes } = compileTrace(parsed.trace);
  // A trace with no replayable action compiles to just the opening navigate.
  // Storing that as READY would present an empty workflow as a result.
  if (steps.length <= 1) return null;
  return { steps, notes };
}
