import { auth } from "@/auth";
import { buildRunView, RUN_TERMINAL_STATUSES } from "@/lib/run-view";

/**
 * Push run updates over SSE instead of the client polling `GET /api/runs/[id]`
 * every 1.5s.
 *
 * `apps/web` deploys to Vercel serverless functions (see `docs/DEPLOY.md`),
 * which have an execution-time limit — 10s by default, and nothing in this
 * app raises it. A run can sit at an approval gate for as long as a human
 * takes, so a single connection held open for a run's whole lifetime would
 * get killed mid-stream in production; nothing here may assume otherwise.
 *
 * Each connection is therefore bounded to `SEGMENT_MS`, comfortably under
 * that limit, and simply ends. The browser's native `EventSource` reconnects
 * on its own the moment a stream closes without an explicit error, so from
 * the client's perspective this is one continuous stream stitched from many
 * short-lived function invocations — safe on serverless, and still real:
 * within a segment, updates push at `POLL_MS` rather than the old 1.5s.
 */
const POLL_MS = 400;
const SEGMENT_MS = 8_000;

function sseEvent(data: unknown): string {
  return `data: ${JSON.stringify(data)}\n\n`;
}

export async function GET(req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return new Response("unauthorized", { status: 401 });
  }
  const orgId = session.user.orgId;
  const viewerId = session.user.id;
  const { id } = await params;

  const encoder = new TextEncoder();
  let lastSent: string | null = null;

  const stream = new ReadableStream<Uint8Array>({
    async start(controller) {
      const deadline = Date.now() + SEGMENT_MS;
      let closed = false;
      const onAbort = () => {
        closed = true;
      };
      req.signal.addEventListener("abort", onAbort);

      // A disconnect can land between the abort check at the top of the loop
      // and the next enqueue/close — Vercel tearing down the connection, the
      // tab closing mid-await. Both calls fail safely (matching this
      // codebase's `.catch(() => undefined)` convention elsewhere) rather
      // than throwing out of a stream controller nothing downstream expects
      // an error from.
      const safeEnqueue = (chunk: Uint8Array) => {
        try {
          controller.enqueue(chunk);
        } catch {
          closed = true;
        }
      };

      try {
        for (;;) {
          if (closed || req.signal.aborted) return;

          const view = await buildRunView(orgId, viewerId, id);
          if (closed || req.signal.aborted) return;
          if (!view) {
            // Not `{ error: ... }` — `RunView` already has its own nullable
            // `error` field, and reusing that key would make the client's
            // `"error" in data` discriminate nothing (both sides of the
            // union would satisfy it).
            safeEnqueue(encoder.encode(sseEvent({ notFound: true })));
            return;
          }

          const serialized = JSON.stringify(view);
          if (serialized !== lastSent) {
            lastSent = serialized;
            safeEnqueue(encoder.encode(sseEvent(view)));
          }

          // Once the run has settled, this segment's final send already
          // carries that state — closing now (rather than waiting out the
          // rest of the segment) lets EventSource's reconnect back off
          // instead of opening one more connection for nothing.
          if (RUN_TERMINAL_STATUSES.has(view.status)) return;

          if (Date.now() >= deadline) return;
          await new Promise((resolve) => setTimeout(resolve, POLL_MS));
        }
      } finally {
        req.signal.removeEventListener("abort", onAbort);
        try {
          controller.close();
        } catch {
          // Already closed — by us above, by the client disconnecting, or by
          // the runtime tearing down the response. Nothing left to do.
        }
      }
    },
  });

  return new Response(stream, {
    headers: {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache, no-transform",
      Connection: "keep-alive",
    },
  });
}
