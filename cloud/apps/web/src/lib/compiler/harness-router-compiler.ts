import { randomUUID } from "node:crypto";
import { artifactStore } from "@ghost/core/storage/artifacts";
import { authoredSteps, formatIssues } from "@/lib/workflow-input";
import {
  cancelSession,
  createHarness,
  downloadSessionFile,
  getSession,
  getSessionFiles,
  HarnessRouterError,
  isSessionTerminal,
  listHarnesses,
  SESSION_SUCCESS_STATUS,
  startResponse,
  uploadFile,
  type HarnessRecord,
} from "@/lib/harness-router";
import {
  CompilerRequestError,
  type CompileJob,
  type CompileProgress,
  type RecordingTrace,
  type WorkflowCompiler,
} from "./types";

/**
 * `WorkflowCompiler` backed by HarnessRouter — **development / self-host
 * only**.
 *
 * This file is the entire HarnessRouter surface as far as the rest of the app
 * is concerned: nothing outside `lib/compiler/` and `lib/harness-router.ts`
 * imports a HarnessRouter type, mentions a harness or a session, or catches a
 * `HarnessRouterError`. Deleting these two files and the one line naming this
 * implementation in `lib/compiler/index.ts` removes the dependency, and the
 * only thing that may break is compilation.
 *
 * See `docs/DEPLOY.md` for why production leaves it unconfigured.
 */

const HARNESS_NAME = "Ghost Recording Compiler";
const STEPS_FILENAME = "steps.json";
const NOTES_FILENAME = "notes.md";
const MAX_NOTES_CHARS = 5_000;

function systemPrompt(): string {
  return `You convert one demonstrated browser-workflow recording into a typed, editable step plan for the Ghost product. You do not build the Ghost product, its UI, its API routes, or its compiler integration, and you never execute the workflow yourself. If asked to do host-product or integration work, respond with a JSON object {"role_mismatch": true, "reason": "<short reason>"} as ${STEPS_FILENAME} instead of steps, and stop.

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
async function ensureHarness(): Promise<string> {
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

/** Test seam: the memo is process-wide, which makes it leak between tests. */
export function __resetHarnessMemo(): void {
  cachedHarnessId = null;
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

/** Vendor errors become the neutral boundary error, so no route has to know
 * what a HarnessRouter is in order to render a sensible status. */
async function translating<T>(fn: () => Promise<T>): Promise<T> {
  try {
    return await fn();
  } catch (err) {
    if (err instanceof HarnessRouterError) {
      throw new CompilerRequestError(err.message, err.status);
    }
    throw err;
  }
}

export const harnessRouterCompiler: WorkflowCompiler = {
  name: "harness-router",

  async start(trace: RecordingTrace): Promise<CompileJob> {
    return translating(async () => {
      const harnessId = await ensureHarness();
      const bytes = await artifactStore().get(trace.traceKey);
      const uploaded = await uploadFile(
        bytes,
        trace.filename,
        guessContentType(trace.filename),
      );

      const { responseId, sessionId } = await startResponse({
        harnessId,
        input: `Convert the attached workflow-recording trace ("${trace.filename}") into ${STEPS_FILENAME}.`,
        idempotencyKey: randomUUID(),
        fileIds: [uploaded.fileId],
      });
      return { jobId: responseId, sessionId };
    });
  },

  async resume(job: CompileJob): Promise<CompileJob> {
    return translating(async () => {
      const harnessId = await ensureHarness();
      const { responseId, sessionId } = await startResponse({
        harnessId,
        input: "Continue where you left off and finish producing steps.json.",
        idempotencyKey: randomUUID(),
        previousResponseId: job.jobId,
        sessionId: job.sessionId,
      });
      return { jobId: responseId, sessionId };
    });
  },

  async cancel(job: CompileJob): Promise<void> {
    await translating(() => cancelSession(job.sessionId));
  },

  async poll(job: CompileJob): Promise<CompileProgress> {
    return translating(async () => {
      const session = await getSession(job.sessionId);
      if (!isSessionTerminal(session.status)) return { state: "running" };

      if (session.status !== SESSION_SUCCESS_STATUS) {
        return {
          state: "failed",
          reason: `The compiler run ended "${session.status}" (turn status "${session.turn_status}").`,
        };
      }

      const files = await getSessionFiles(job.sessionId, { changed: true });

      // Notes are read first so they survive every failure path below: they
      // are the compiler's own account of what it could not resolve, and they
      // are most useful precisely when it did not succeed.
      let notes: string | null = null;
      const notesFile = files.find((f) => f.path.toLowerCase().endsWith(NOTES_FILENAME));
      if (notesFile) {
        const { buffer } = await downloadSessionFile(job.sessionId, notesFile.file_id);
        notes = buffer.toString("utf8").slice(0, MAX_NOTES_CHARS);
      }

      const stepsFile = files.find((f) => f.path.toLowerCase().endsWith(STEPS_FILENAME));
      if (!stepsFile) {
        return {
          state: "failed",
          reason: `The compiler finished without producing ${STEPS_FILENAME}.`,
          notes,
        };
      }

      const { buffer: stepsBuffer } = await downloadSessionFile(
        job.sessionId,
        stepsFile.file_id,
      );

      let rawSteps: unknown;
      try {
        rawSteps = JSON.parse(stepsBuffer.toString("utf8"));
      } catch {
        return { state: "failed", reason: `${STEPS_FILENAME} was not valid JSON.`, notes };
      }

      if (
        rawSteps &&
        typeof rawSteps === "object" &&
        !Array.isArray(rawSteps) &&
        "role_mismatch" in (rawSteps as Record<string, unknown>)
      ) {
        const reason = (rawSteps as { reason?: string }).reason ?? "role mismatch";
        return { state: "failed", reason: `The compiler declined this task: ${reason}`, notes };
      }

      // "No steps" is a real answer, not a malformed one: the instructions
      // invite an empty array when a trace holds no discernible browser
      // actions. Saying so plainly — and keeping the compiler's own notes,
      // which explain why — beats letting it fall through to `authoredSteps`
      // and surface as the schema's "a workflow needs at least one step",
      // which describes the validator's rule rather than what actually
      // happened to the recording.
      if (Array.isArray(rawSteps) && rawSteps.length === 0) {
        return {
          state: "failed",
          reason:
            "The compiler found no browser actions in this trace, so there is nothing to publish." +
            (notes ? " See its notes below for what it looked for." : ""),
          notes,
        };
      }

      const parsed = authoredSteps.safeParse(rawSteps);
      if (!parsed.success) {
        return {
          state: "failed",
          reason: `Proposed steps failed validation: ${formatIssues(parsed.error)}`,
          notes,
        };
      }

      return { state: "ready", steps: parsed.data, notes };
    });
  },
};
