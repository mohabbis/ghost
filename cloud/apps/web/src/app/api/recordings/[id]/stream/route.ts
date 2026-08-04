import { auth } from "@/auth";
import { buildRecordingView } from "@/lib/recording-view";

/**
 * Push compile-status updates over SSE. Same shape as
 * `apps/web/src/app/api/runs/[id]/stream/route.ts` — see that file for why
 * the connection is deliberately bounded to `SEGMENT_MS` rather than held
 * open for the whole compile (Vercel serverless execution limits).
 *
 * Polling `buildRecordingView` this way, rather than opening a second SSE
 * connection to HarnessRouter, keeps the "hold a stream open across a
 * serverless boundary" problem in exactly one place in this codebase.
 */
const POLL_MS = 1_500;
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

          const view = await buildRecordingView(orgId, id);
          if (closed || req.signal.aborted) return;
          if (!view) {
            safeEnqueue(encoder.encode(sseEvent({ notFound: true })));
            return;
          }

          const serialized = JSON.stringify(view);
          if (serialized !== lastSent) {
            lastSent = serialized;
            safeEnqueue(encoder.encode(sseEvent(view)));
          }

          if (view.compileStatus !== "RUNNING") return;

          if (Date.now() >= deadline) return;
          await new Promise((resolve) => setTimeout(resolve, POLL_MS));
        }
      } finally {
        req.signal.removeEventListener("abort", onAbort);
        try {
          controller.close();
        } catch {
          // Already closed.
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
