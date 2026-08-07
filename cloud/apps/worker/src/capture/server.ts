import { createServer, type Server } from "node:http";
import { WebSocketServer, type WebSocket } from "ws";
import {
  CAPTURE_LIMITS,
  captureConfigured,
  verifyCaptureTicket,
  type CaptureClientMessage,
  type CaptureServerMessage,
  type CaptureTicketClaims,
} from "@ghost/core/recording/capture";
import { createLogger, serializeError } from "@ghost/core/logger";
import { captureException } from "@ghost/core/sentry";
import { CaptureSession } from "./session.js";

const log = createLogger("capture-server");

/**
 * The socket the user's browser drives a capture session over.
 *
 * It lives in the worker because that is where a browser can live: `apps/web`
 * runs on request-scoped serverless functions, and a person demonstrating a
 * workflow holds a page for minutes. So the worker — already long-running,
 * already holding Playwright for replay — accepts one WebSocket per session
 * and pumps it into a `CaptureSession`.
 *
 * ## Authentication
 *
 * The worker has no session table and no way to ask the web app who is
 * connecting, so it does not try: the web app signs a short-lived ticket with a
 * key both processes hold, and this server verifies it offline. The ticket
 * arrives as the **first message on the socket** rather than in the URL, so it
 * never reaches an access log or a proxy trace. A socket that has not
 * authenticated within `authTimeoutMs` is dropped, and an unauthenticated
 * socket can do nothing else at all — there is no state for it to touch.
 *
 * With no `GHOST_CAPTURE_KEY` set, this server does not listen. That is the
 * whole feature switch: not "capture but unauthenticated", which is a browser
 * anyone who can reach the port may drive.
 */

export interface CaptureServer {
  port: number;
  close(): Promise<void>;
}

/** Sessions currently holding a browser. Bounded by `maxConcurrentSessions`. */
const live = new Set<CaptureSession>();
/** Sessions whose browser is still starting — they count against the cap too. */
let opening = 0;

export function liveCaptureSessions(): number {
  return live.size;
}

export function startCaptureServer(): CaptureServer | null {
  if (!captureConfigured()) {
    log.info("remote capture disabled (GHOST_CAPTURE_KEY not set)");
    return null;
  }

  const port = Number(process.env.GHOST_CAPTURE_PORT ?? 8787);
  const http = createServer((req, res) => {
    // A health endpoint, because a container that cannot be probed gets
    // restarted by its scheduler at the worst possible moment.
    if (req.url === "/health") {
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: true, sessions: live.size, max: CAPTURE_LIMITS.maxConcurrentSessions }));
      return;
    }
    res.writeHead(404).end();
  });

  const wss = new WebSocketServer({
    server: http,
    path: "/capture",
    // Input messages are small; the ceiling exists so a malformed or hostile
    // client cannot hand the worker a 100MB string to parse.
    maxPayload: 256 * 1024,
  });

  wss.on("connection", (socket) => void accept(socket));
  http.listen(port, () => log.info("capture server listening", { port }));

  return {
    port,
    close: async () => {
      // End sessions before closing the server: each holds a browser process,
      // and a container that exits without closing them leaks them.
      await Promise.all([...live].map((s) => s.finish("disconnected").catch(() => undefined)));
      wss.close();
      await new Promise<void>((resolve) => http.close(() => resolve()));
    },
  };
}

async function accept(socket: WebSocket): Promise<void> {
  let session: CaptureSession | null = null;

  const send = (msg: CaptureServerMessage): void => {
    if (socket.readyState !== socket.OPEN) return;
    // Frames are droppable and everything else is not. A client on a slow link
    // would otherwise accumulate a frame backlog in the worker's memory, and a
    // frame that arrives late is worthless anyway — the next one is right
    // behind it. Control messages are never dropped: `stopped` carries the
    // recording id, and losing it would strand a recording the user made.
    if (msg.t === "frame" && socket.bufferedAmount > CAPTURE_LIMITS.maxBufferedBytes) return;
    socket.send(JSON.stringify(msg));
  };

  const fail = (message: string): void => {
    send({ t: "error", message });
    socket.close();
  };

  // Nothing may happen on this socket until it proves who it is.
  const authTimer = setTimeout(() => {
    if (!session) fail("Capture session did not authenticate in time.");
  }, CAPTURE_LIMITS.authTimeoutMs);
  authTimer.unref();

  socket.on("message", (raw) => {
    let msg: CaptureClientMessage;
    try {
      msg = JSON.parse(String(raw)) as CaptureClientMessage;
    } catch {
      fail("Unreadable message.");
      return;
    }

    if (!session) {
      if (msg.t !== "auth") {
        // Deliberately refused rather than buffered. Accepting input for a
        // session that does not exist yet is how an unauthenticated socket
        // gets to influence one that later does.
        fail("The first message on a capture socket must be an auth ticket.");
        return;
      }
      clearTimeout(authTimer);
      void open(msg.ticket);
      return;
    }

    // Every handler is a browser operation and may reject; a rejection must
    // close the session rather than become an unhandled rejection that takes
    // the worker down with replay still running on it.
    void session.handle(msg).catch((err: unknown) => {
      log.error("capture input failed", serializeError(err));
      send({ t: "error", message: "That action could not be sent to the browser." });
    });
  });

  socket.on("close", () => {
    clearTimeout(authTimer);
    void session?.finish("disconnected").catch(() => undefined);
  });
  socket.on("error", (err) => log.warn("capture socket error", serializeError(err)));

  async function open(ticket: string): Promise<void> {
    const verified = verifyCaptureTicket(ticket);
    if (!verified.ok) {
      log.warn("capture ticket rejected", { reason: verified.reason });
      fail("This capture session is no longer valid. Start a new recording.");
      return;
    }

    // Counted *before* the browser launches, and including sessions still
    // launching. Checking `live.size` alone would let every socket that
    // arrived during a slow Chromium start find room, and the cap would hold
    // everywhere except under the load it exists for.
    if (live.size + opening >= CAPTURE_LIMITS.maxConcurrentSessions) {
      // Refused with a reason a person can act on, rather than a browser
      // launched into a worker that has no memory left for it.
      fail(
        "Ghost is already running as many recording sessions as this worker allows. " +
          "Try again in a few minutes.",
      );
      return;
    }

    const claims: CaptureTicketClaims = verified.claims;
    opening++;
    try {
      const opened = await CaptureSession.open(claims, send, () => {
        live.delete(opened);
        if (socket.readyState === socket.OPEN) socket.close();
      });
      session = opened;
      live.add(opened);
      log.info("capture session opened", {
        sessionId: claims.sessionId,
        orgId: claims.orgId,
        sessions: live.size,
      });
    } catch (err) {
      log.error("capture session failed to open", serializeError(err));
      captureException(err, { phase: "capture.open", sessionId: claims.sessionId });
      fail("Ghost could not start a browser for this recording. Try again.");
    } finally {
      opening--;
    }
  }
}
