/**
 * Server-only HarnessRouter client.
 *
 * HarnessRouter runs the configured runtime agent that turns one uploaded
 * recording trace into typed workflow steps (see recording-compiler.ts). This
 * module owns every network call to it; nothing here may run in a Client
 * Component, and `HR_API_KEY` must never reach the browser — every function
 * below is imported only from route handlers and server-only lib code.
 *
 * Request bodies are snake_case; harness records come back camelCase; session
 * and file objects come back snake_case. That inconsistency is HarnessRouter's,
 * not a typo here — see the per-type comments.
 */

const HR_BASE_URL = "https://api.harnessrouter.ai";

function apiKey(): string {
  const key = process.env.HR_API_KEY;
  if (!key) throw new Error("HR_API_KEY is not configured");
  return key;
}

export class HarnessRouterError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
    this.name = "HarnessRouterError";
  }
}

async function hrRequest(path: string, init: RequestInit = {}): Promise<Response> {
  const res = await fetch(`${HR_BASE_URL}${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${apiKey()}`,
      ...init.headers,
    },
  });
  return res;
}

async function hrJson<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await hrRequest(path, {
    ...init,
    headers: { "Content-Type": "application/json", ...init.headers },
  });
  if (!res.ok) {
    const body = (await res.json().catch(() => null)) as { detail?: string } | null;
    throw new HarnessRouterError(body?.detail ?? `HarnessRouter request failed`, res.status);
  }
  return (await res.json()) as T;
}

// --- Harness (configured agent) records — camelCase ------------------------

export interface HarnessRecord {
  id: string;
  name: string;
  base: "codex" | "claude-code" | "hermes";
  defaultModel: string | null;
  systemPrompt: string;
  mcpServers: unknown[];
  skills: unknown[];
}

export async function listHarnesses(): Promise<HarnessRecord[]> {
  const res = await hrJson<{ harnesses?: HarnessRecord[] } | HarnessRecord[]>("/v1/harnesses");
  return Array.isArray(res) ? res : (res.harnesses ?? []);
}

export async function createHarness(input: {
  name: string;
  base: "codex" | "claude-code" | "hermes";
  default_model?: string;
  system_prompt: string;
  mcp_servers?: unknown[];
  skills?: unknown[];
}): Promise<HarnessRecord> {
  return hrJson<HarnessRecord>("/v1/harnesses", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

// --- Models ------------------------------------------------------------

export interface ModelInfo {
  id: string;
  label: string;
  backend: string;
  available: boolean;
  default: boolean;
}

export interface ModelsResponse {
  backends: Record<string, { default: string; models: ModelInfo[] }>;
}

export async function getModels(): Promise<ModelsResponse> {
  return hrJson<ModelsResponse>("/v1/models");
}

// --- Files ---------------------------------------------------------------

/**
 * Uploads one file and returns its id.
 *
 * The upload response names this field **`id`** (`file_*`), not `file_id` —
 * unlike the *session* files API, which returns `file_id` (`cfile_*`) for a
 * different kind of id entirely. Reading the wrong key here does not throw:
 * it yields `undefined`, the `input_file` block is accepted anyway, and the
 * agent simply starts with no attachment and reports it cannot find the file.
 * `uploadFile.test.ts` pins the mapping for that reason.
 */
export async function uploadFile(
  bytes: Buffer,
  filename: string,
  contentType: string,
): Promise<{ fileId: string }> {
  const form = new FormData();
  form.append("file", new Blob([new Uint8Array(bytes)], { type: contentType }), filename);
  const res = await hrRequest("/v1/files", { method: "POST", body: form });
  if (!res.ok) {
    const body = (await res.json().catch(() => null)) as { detail?: string } | null;
    throw new HarnessRouterError(body?.detail ?? "HarnessRouter file upload failed", res.status);
  }
  const body = (await res.json()) as { id?: string };
  if (!body.id) {
    throw new HarnessRouterError("HarnessRouter upload returned no file id", 502);
  }
  return { fileId: body.id };
}

// --- Sessions — snake_case -------------------------------------------------

export interface SessionRecord {
  session_id: string;
  status: string;
  turn_status: string;
  last_response_id: string | null;
  harness_id: string;
  harness_name: string;
  model: string;
  backend: string;
  title: string | null;
  elapsed: number | null;
  finished_at: string | null;
  event_count: number;
}

/** Success-side terminal per the wire contract: "done", not "completed". */
export const SESSION_SUCCESS_STATUS = "done";
export const SESSION_FAILURE_STATUSES = new Set(["failed", "cancelled", "incomplete"]);
export const SESSION_RUNNING_STATUSES = new Set(["running", "starting"]);

export function isSessionTerminal(status: string): boolean {
  return !SESSION_RUNNING_STATUSES.has(status);
}

export async function getSession(sessionId: string): Promise<SessionRecord> {
  return hrJson<SessionRecord>(`/v1/sessions/${sessionId}`);
}

export async function getSessionTurns(sessionId: string): Promise<{ turns: unknown[] }> {
  return hrJson<{ turns: unknown[] }>(`/v1/sessions/${sessionId}/turns`);
}

export async function cancelSession(sessionId: string): Promise<void> {
  await hrRequest(`/v1/sessions/${sessionId}/cancel`, { method: "POST" });
}

export interface SessionFile {
  path: string;
  bytes: number;
  media_type: string;
  file_id: string;
  download_url: string;
}

export async function getSessionFiles(
  sessionId: string,
  opts: { changed?: boolean } = {},
): Promise<SessionFile[]> {
  const qs = opts.changed ? "?changed=true" : "";
  const res = await hrJson<{ files: SessionFile[] }>(`/v1/sessions/${sessionId}/files${qs}`);
  return res.files;
}

/** Fetches the raw bytes of one session file. Server-side only — this hits an
 * API-key-protected endpoint, never a pre-signed public link. */
export async function downloadSessionFile(
  sessionId: string,
  fileId: string,
): Promise<{ buffer: Buffer; contentType: string }> {
  const res = await hrRequest(`/v1/containers/${sessionId}/files/${fileId}/content`);
  if (!res.ok) {
    throw new HarnessRouterError(`HarnessRouter file download failed`, res.status);
  }
  const contentType = res.headers.get("content-type") ?? "application/octet-stream";
  const buffer = Buffer.from(await res.arrayBuffer());
  return { buffer, contentType };
}

// --- Responses (streaming) --------------------------------------------------

/**
 * Server-Sent Events, OpenAI Responses style: `data:`-only frames, no `event:`
 * lines. The event name is `type` *inside* the JSON payload. Unknown `type`
 * values are ignored, not errors — the set is open.
 */
type StreamFrame = { type: string; [key: string]: unknown };

async function* parseSseFrames(body: ReadableStream<Uint8Array>): AsyncGenerator<StreamFrame> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buf = "";
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) return;
      buf += decoder.decode(value, { stream: true });
      let sep: number;
      while ((sep = buf.indexOf("\n\n")) !== -1) {
        const rawEvent = buf.slice(0, sep);
        buf = buf.slice(sep + 2);
        const dataLine = rawEvent
          .split("\n")
          .find((line) => line.startsWith("data:"));
        if (!dataLine) continue;
        const jsonText = dataLine.slice("data:".length).trim();
        if (!jsonText) continue;
        try {
          yield JSON.parse(jsonText) as StreamFrame;
        } catch {
          // Malformed frame — skip rather than abort the whole stream.
        }
      }
    }
  } finally {
    // `cancel`, not `releaseLock` — callers routinely stop consuming as soon
    // as they have `response.created` (the run keeps going server-side
    // either way), and cancelling is what actually closes the socket instead
    // of leaving it open until GC gets to it.
    await reader.cancel().catch(() => undefined);
  }
}

export interface StartResponseInput {
  harnessId: string;
  input: string;
  idempotencyKey: string;
  fileIds?: string[];
  previousResponseId?: string;
  sessionId?: string;
}

/**
 * Starts (or continues) a runtime task and returns as soon as the recovery
 * identifiers are known. Deliberately does not wait for the run to finish —
 * closing this connection does not stop the run (it keeps executing
 * server-side), so holding it open for the whole task inside a
 * time-limited serverless function would only waste the invocation. Callers
 * recover progress by polling `getSession`.
 */
export async function startResponse(
  input: StartResponseInput,
): Promise<{ responseId: string; sessionId: string }> {
  const content: unknown[] = [{ type: "input_text", text: input.input }];
  for (const fileId of input.fileIds ?? []) {
    content.push({ type: "input_file", file_id: fileId });
  }

  const body: Record<string, unknown> = {
    input: [{ role: "user", content }],
    stream: true,
  };
  if (input.previousResponseId) body.previous_response_id = input.previousResponseId;
  if (input.sessionId) body.metadata = { session_id: input.sessionId };

  const res = await hrRequest(`/${input.harnessId}/v1/responses`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Idempotency-Key": input.idempotencyKey,
    },
    body: JSON.stringify(body),
  });
  if (!res.ok || !res.body) {
    const errBody = (await res.json().catch(() => null)) as { detail?: string } | null;
    throw new HarnessRouterError(
      errBody?.detail ?? "HarnessRouter failed to start the run",
      res.status,
    );
  }

  for await (const frame of parseSseFrames(res.body)) {
    if (frame.type !== "response.created") continue;
    const response = frame.response as
      | { id?: string; metadata?: { session_id?: string } }
      | undefined;
    const responseId = response?.id;
    const sessionId = response?.metadata?.session_id;
    if (responseId && sessionId) {
      return { responseId, sessionId };
    }
  }
  throw new HarnessRouterError("HarnessRouter stream ended before response.created", 502);
}
