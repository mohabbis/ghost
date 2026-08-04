import { resolveAgentPrincipal } from "@/lib/agent-auth";
import { ingestTrace, MAX_TRACE_BYTES } from "@/lib/recording-ingest";

/**
 * Recording ingest for non-interactive clients — the Chrome extension.
 *
 * Lives under `/api/agent` because that is the surface authenticated by a
 * revocable bearer credential rather than a browser session. The extension
 * runs outside Ghost's origin and cannot rely on the session cookie reaching
 * it, so it holds a token the user creates in Settings and can revoke there.
 *
 * It sits on the "propose" side of the trust boundary, like every other agent
 * route: uploading a recording creates a `Recording` whose compiled steps are
 * a *proposal*. Nothing here publishes a workflow or executes anything. The
 * human reviews the steps in the editor and publishes through
 * `POST /api/workflows`, which revalidates them.
 *
 * `resolveAgentPrincipal` accepts a session too, and enforces the second
 * factor on that path — see `lib/agent-auth.ts`.
 *
 * Body is JSON rather than multipart: the extension builds the trace in
 * memory and has no file to attach.
 */
export async function POST(req: Request) {
  const principal = await resolveAgentPrincipal(req);
  if (!principal.ok) {
    return Response.json({ error: principal.error }, { status: principal.status });
  }

  const raw = await req.text();
  if (!raw) {
    return Response.json({ error: "a JSON recording trace is required" }, { status: 400 });
  }
  if (Buffer.byteLength(raw) > MAX_TRACE_BYTES) {
    return Response.json(
      { error: `trace exceeds the ${MAX_TRACE_BYTES / (1024 * 1024)}MB limit` },
      { status: 413 },
    );
  }

  // Parsed only to reject obvious junk before it reaches storage. The
  // authoritative validation is `parseRecordingTrace` inside `ingestTrace`,
  // which decides whether this is a Ghost trace worth compiling.
  try {
    JSON.parse(raw);
  } catch {
    return Response.json({ error: "body was not valid JSON" }, { status: 400 });
  }

  const result = await ingestTrace({
    orgId: principal.principal.orgId,
    userId: principal.principal.userId,
    filename: "extension-trace.json",
    contentType: "application/json",
    buffer: Buffer.from(raw, "utf8"),
  });

  return Response.json(result, { status: 201 });
}
