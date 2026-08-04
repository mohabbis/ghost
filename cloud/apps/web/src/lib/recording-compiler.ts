import { randomUUID } from "node:crypto";
import { artifactStore } from "@ghost/core/storage/artifacts";
import { appendAuditEvent } from "@ghost/core/audit-log";
import type { WorkflowSteps } from "@ghost/core/schema/step";
import { Prisma, type RecordingCompileStatus } from "@ghost/core/db";
import { prisma } from "@/lib/db";
import { authoredSteps, formatIssues } from "@/lib/workflow-input";
import {
  cancelSession,
  createHarness,
  downloadSessionFile,
  getSession,
  getSessionFiles,
  isSessionTerminal,
  listHarnesses,
  SESSION_SUCCESS_STATUS,
  startResponse,
  uploadFile,
  type HarnessRecord,
} from "@/lib/harness-router";

/**
 * Turns one uploaded workflow-recording trace into a proposed, editable
 * `WorkflowStep[]` via a purpose-configured HarnessRouter agent.
 *
 * This is the "Convert" half of Ghost's Phase 2 recording pillar
 * (`docs/CURSOR_HANDOFF.md`). The agent's output is a *proposal* — it lands in
 * `Recording.compiledSteps` and is never executed. A human reviews it in the
 * same `WorkflowEditor` used for hand-authored workflows and explicitly
 * publishes it through the existing `POST /api/workflows` path, which
 * re-validates every step. Nothing here bypasses that trust pipeline.
 */

const HARNESS_NAME = "Ghost Recording Compiler";
const STEPS_FILENAME = "steps.json";
const NOTES_FILENAME = "notes.md";
const MAX_NOTES_CHARS = 5_000;

function systemPrompt(): string {
  return `You convert one demonstrated browser-workflow recording into a typed, editable step plan for the Ghost product. You do not build the Ghost product, its UI, its API routes, or its HarnessRouter integration, and you never execute the workflow yourself. If asked to do host-product or integration work, respond with a JSON object {"role_mismatch": true, "reason": "<short reason>"} as ${STEPS_FILENAME} instead of steps, and stop.

You will receive one attached file: the raw trace of a recorded browser session (this may be a JSON event log, a HAR file, a Playwright trace .zip, or similar — inspect it, unzip it if needed).

Produce exactly one file, ${STEPS_FILENAME}, containing a JSON array of steps. Each step is one JSON object matching exactly one of these shapes (fields beyond "type" and the ones listed are optional and may be omitted):

- {"type":"navigate","id":"<unique id>","url":"<absolute url>"}
- {"type":"click","id":"<unique id>","selector":{"role"?,"name"?,"testId"?,"text"?,"css"?},"description"?}
- {"type":"fill","id":"<unique id>","selector":{...},"value":"<string>","sensitive"?:true}
- {"type":"select","id":"<unique id>","selector":{...},"value":"<string>"}
- {"type":"waitFor","id":"<unique id>","selector"?:{...},"urlPattern"?,"ms"?}
- {"type":"extract","id":"<unique id>","selector":{...},"name":"<name to store the value under>"}
- {"type":"verify","id":"<unique id>","assertion":{"kind":"url","expected":"..."} | {"kind":"selectorVisible","selector":{...}} | {"kind":"textPresent","expected":"..."}}
- {"type":"approval","id":"<unique id>","reason":"<why a human must approve here>"}

Rules, all mandatory:

1. Every "id" is a short unique string stable within the file.
2. selectors: prefer "role"+"name" (ARIA role and accessible name) or "testId" over raw "css". Never emit pixel coordinates — there is no step field for them.
3. Set "sensitive": true on any "fill" step whose value is a password, OTP, card number, or other secret/PII the recording captured. Never copy the actual secret value into the step — replace it with a short placeholder like "<password>".
4. Insert an "approval" step immediately before any action that sends, pays, deletes, or submits something irreversible, with "reason" explaining what is about to happen.
5. Output ONLY the steps array in ${STEPS_FILENAME} — no surrounding prose, no markdown fences, valid JSON, minified or pretty is fine.
6. Optionally also write ${NOTES_FILENAME} (plain text or markdown, under 2000 words) explaining anything you were unsure about or could not resolve from the trace — this is shown to the human reviewing your proposal, so name concrete uncertainties, not generic caveats.
7. Never fetch a network resource, call an external service, or read anything other than the attached trace file.
8. Never return a localhost or 127.0.0.1 link anywhere in your output.

If the trace is empty, corrupt, or contains no discernible browser actions, still write ${STEPS_FILENAME} as a JSON array containing zero or more "verify"/"approval" steps as appropriate, and use ${NOTES_FILENAME} to say exactly what was wrong with the input.`;
}

let cachedHarnessId: string | null = null;

/** Find-or-create the configured agent. Cheap and idempotent — safe to call
 * on every compile start; memoized in-process so repeated calls in the same
 * server instance skip the `GET /v1/harnesses` round trip. */
export async function ensureRecordingCompilerHarness(): Promise<string> {
  if (cachedHarnessId) return cachedHarnessId;

  const existing = await listHarnesses();
  const match = existing.find((h: HarnessRecord) => h.name === HARNESS_NAME);
  if (match) {
    cachedHarnessId = match.id;
    return match.id;
  }

  const created = await createHarness({
    name: HARNESS_NAME,
    base: "claude-code",
    system_prompt: systemPrompt(),
    mcp_servers: [],
    skills: [],
  });
  cachedHarnessId = created.id;
  return created.id;
}

function guessContentType(filename: string): string {
  const ext = filename.toLowerCase().split(".").pop();
  switch (ext) {
    case "json":
      return "application/json";
    case "har":
      return "application/json";
    case "zip":
      return "application/zip";
    case "txt":
      return "text/plain";
    default:
      return "application/octet-stream";
  }
}

export interface RecordingForCompile {
  id: string;
  orgId: string;
  rawTraceKey: string | null;
  rawTraceFilename: string | null;
}

/** Starts a compile task and returns the recovery identifiers. Does not wait
 * for completion — see harness-router.ts `startResponse`. */
export async function startCompile(
  recording: RecordingForCompile,
): Promise<{ responseId: string; sessionId: string }> {
  if (!recording.rawTraceKey) {
    throw new Error("recording has no uploaded trace to compile");
  }
  const harnessId = await ensureRecordingCompilerHarness();
  const bytes = await artifactStore().get(recording.rawTraceKey);
  const filename = recording.rawTraceFilename ?? "recording-trace";
  const uploaded = await uploadFile(bytes, filename, guessContentType(filename));

  return startResponse({
    harnessId,
    input: `Convert the attached workflow-recording trace ("${filename}") into ${STEPS_FILENAME}.`,
    idempotencyKey: randomUUID(),
    fileIds: [uploaded.fileId],
  });
}

/** Continues a compile session that ended `incomplete`. */
export async function continueCompile(
  harnessId: string,
  sessionId: string,
  previousResponseId: string,
): Promise<{ responseId: string; sessionId: string }> {
  return startResponse({
    harnessId,
    input: "Continue where you left off and finish producing steps.json.",
    idempotencyKey: randomUUID(),
    previousResponseId,
    sessionId,
  });
}

export async function cancelCompile(sessionId: string): Promise<void> {
  await cancelSession(sessionId);
}

interface FinalizeResult {
  /** `null` when the recording does not exist (already deleted, or a bad id). */
  compileStatus: RecordingCompileStatus | null;
  terminal: boolean;
}

/**
 * Polls the HarnessRouter session for one recording and, the first time it
 * observes a terminal state, downloads and validates the result and persists
 * it. Idempotent to call repeatedly while `RUNNING` — a no-op once the
 * recording's `compileStatus` has already left `RUNNING`.
 */
export async function finalizeIfTerminal(recordingId: string): Promise<FinalizeResult> {
  const recording = await prisma.recording.findUnique({
    where: { id: recordingId },
    select: {
      id: true,
      orgId: true,
      compileStatus: true,
      harnessSessionId: true,
    },
  });
  if (!recording) return { compileStatus: null, terminal: true };
  if (recording.compileStatus !== "RUNNING" || !recording.harnessSessionId) {
    return { compileStatus: recording.compileStatus, terminal: true };
  }

  const session = await getSession(recording.harnessSessionId);
  if (!isSessionTerminal(session.status)) {
    return { compileStatus: "RUNNING", terminal: false };
  }

  if (session.status !== SESSION_SUCCESS_STATUS) {
    const compileStatus = await persistOutcome(recording.id, recording.orgId, {
      compileStatus: "FAILED",
      compileError: `HarnessRouter run ended "${session.status}" (turn status "${session.turn_status}").`,
    });
    return { compileStatus, terminal: true };
  }

  const files = await getSessionFiles(recording.harnessSessionId, { changed: true });
  const stepsFile = files.find((f) => f.path.toLowerCase().endsWith(STEPS_FILENAME));
  if (!stepsFile) {
    const compileStatus = await persistOutcome(recording.id, recording.orgId, {
      compileStatus: "FAILED",
      compileError: `The compiler finished without producing ${STEPS_FILENAME}.`,
    });
    return { compileStatus, terminal: true };
  }

  const { buffer: stepsBuffer } = await downloadSessionFile(
    recording.harnessSessionId,
    stepsFile.file_id,
  );

  let rawSteps: unknown;
  try {
    rawSteps = JSON.parse(stepsBuffer.toString("utf8"));
  } catch {
    const compileStatus = await persistOutcome(recording.id, recording.orgId, {
      compileStatus: "FAILED",
      compileError: `${STEPS_FILENAME} was not valid JSON.`,
    });
    return { compileStatus, terminal: true };
  }

  if (
    rawSteps &&
    typeof rawSteps === "object" &&
    !Array.isArray(rawSteps) &&
    "role_mismatch" in (rawSteps as Record<string, unknown>)
  ) {
    const reason = (rawSteps as { reason?: string }).reason ?? "role mismatch";
    const compileStatus = await persistOutcome(recording.id, recording.orgId, {
      compileStatus: "FAILED",
      compileError: `The compiler declined this task: ${reason}`,
    });
    return { compileStatus, terminal: true };
  }

  let notes: string | null = null;
  const notesFile = files.find((f) => f.path.toLowerCase().endsWith(NOTES_FILENAME));
  if (notesFile) {
    const { buffer } = await downloadSessionFile(recording.harnessSessionId, notesFile.file_id);
    notes = buffer.toString("utf8").slice(0, MAX_NOTES_CHARS);
  }

  // "No steps" is a real answer, not a malformed one: the instructions invite
  // an empty array when a trace holds no discernible browser actions. Saying
  // so plainly — and keeping the compiler's own notes, which explain why —
  // beats letting it fall through to `authoredSteps` and surface as the
  // schema's "a workflow needs at least one step", which describes the
  // validator's rule rather than what actually happened to the recording.
  if (Array.isArray(rawSteps) && rawSteps.length === 0) {
    const compileStatus = await persistOutcome(recording.id, recording.orgId, {
      compileStatus: "FAILED",
      compileNotes: notes,
      compileError:
        "The compiler found no browser actions in this trace, so there is nothing to publish." +
        (notes ? " See its notes below for what it looked for." : ""),
    });
    return { compileStatus, terminal: true };
  }

  const parsed = authoredSteps.safeParse(rawSteps);
  if (!parsed.success) {
    const compileStatus = await persistOutcome(recording.id, recording.orgId, {
      compileStatus: "FAILED",
      compileNotes: notes,
      compileError: `Proposed steps failed validation: ${formatIssues(parsed.error)}`,
    });
    return { compileStatus, terminal: true };
  }

  const compileStatus = await persistOutcome(recording.id, recording.orgId, {
    compileStatus: "READY",
    compiledSteps: parsed.data,
    compileNotes: notes,
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
