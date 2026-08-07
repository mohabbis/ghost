import { createHmac, randomUUID, timingSafeEqual } from "node:crypto";

/**
 * The contract between the web app and the worker's remote capture browser.
 *
 * Recording needs a browser, and the browser cannot live where the UI does:
 * `apps/web` runs on serverless functions that end with the request, while a
 * capture session is a human driving a page for several minutes. So the
 * browser lives in the worker — the same long-running container that already
 * holds Playwright for replay — and the user drives it over a WebSocket.
 *
 * That splits one session across two processes which share no memory, so this
 * module defines the three things both sides must agree on: who is allowed to
 * connect (`mintCaptureTicket` / `verifyCaptureTicket`), what may be said over
 * the socket (the message unions), and when a session is cut off
 * (`CAPTURE_LIMITS`).
 *
 * ## Why a ticket rather than the session cookie
 *
 * The worker has no Auth.js, no session table, and no reason to grow either.
 * The web app is the only thing that knows who is signed in, so it says so
 * once, in a form the worker can check offline: an HMAC over the fields that
 * matter, with a short expiry. The worker holds the same key and verifies; it
 * never calls back into the web app, so a capture cannot be opened by anyone
 * who has not just been authenticated by it.
 *
 * The ticket is deliberately narrow. It authorises **one** capture session for
 * **one** recording in **one** organization, for about as long as it takes a
 * browser to open a socket. It is not a session token: it cannot be replayed
 * into any other endpoint, and it grants nothing after the worker has bound it
 * to a live session.
 */

export const CAPTURE_TICKET_VERSION = "gcap1";

/** What the web app asserts to the worker about a capture session. */
export interface CaptureTicketClaims {
  /**
   * Correlates the session across the two processes and names its trace.
   *
   * Deliberately *not* a `Recording` row id. The row is created by
   * `ingestTrace` at the moment a trace exists, so a capture the user abandons
   * — a closed tab, a session that timed out — leaves nothing behind to
   * explain. The real recording id comes back in the `stopped` message.
   */
  sessionId: string;
  orgId: string;
  /** Who started it, for the audit trail. Null for org-scoped callers. */
  userId: string | null;
  /** Where the browser opens. Bound into the ticket so it cannot be swapped. */
  startUrl: string;
  /** Epoch seconds. */
  exp: number;
}

export class CaptureKeyMissingError extends Error {
  constructor() {
    super(
      "GHOST_CAPTURE_KEY is not set. Generate one with `openssl rand -base64 32` and set the " +
        "same value on both the web app and the worker — it is what lets the worker trust that " +
        "a capture session belongs to a signed-in user.",
    );
    this.name = "CaptureKeyMissingError";
  }
}

/**
 * Whether remote capture is available at all.
 *
 * Absent key means the feature is off, not degraded: the UI does not offer it
 * and the worker does not listen. Ghost refuses to accept unauthenticated
 * capture connections rather than quietly running a browser for anyone who can
 * reach the port.
 */
export function captureConfigured(): boolean {
  return Boolean(process.env.GHOST_CAPTURE_KEY);
}

function captureKey(): Buffer {
  const raw = process.env.GHOST_CAPTURE_KEY;
  if (!raw) throw new CaptureKeyMissingError();
  return Buffer.from(raw, "utf8");
}

function b64url(input: Buffer): string {
  return input.toString("base64url");
}

/**
 * How long a freshly minted ticket may sit unused.
 *
 * Short on purpose: the only gap it has to cover is the browser opening a
 * WebSocket to a URL it was just handed. Anything longer is a bearer token
 * sitting in a page's memory for no reason.
 */
export const CAPTURE_TICKET_TTL_SECONDS = 120;

/** A fresh correlation id for one capture session. */
export function newCaptureSessionId(): string {
  return randomUUID();
}

export function mintCaptureTicket(
  claims: Omit<CaptureTicketClaims, "exp"> & { exp?: number },
): string {
  const payload: CaptureTicketClaims = {
    sessionId: claims.sessionId,
    orgId: claims.orgId,
    userId: claims.userId,
    startUrl: claims.startUrl,
    exp: claims.exp ?? Math.floor(Date.now() / 1000) + CAPTURE_TICKET_TTL_SECONDS,
  };
  const body = b64url(Buffer.from(JSON.stringify(payload), "utf8"));
  const mac = b64url(
    createHmac("sha256", captureKey()).update(`${CAPTURE_TICKET_VERSION}.${body}`).digest(),
  );
  return `${CAPTURE_TICKET_VERSION}.${body}.${mac}`;
}

export type CaptureTicketResult =
  | { ok: true; claims: CaptureTicketClaims }
  | { ok: false; reason: string };

export function verifyCaptureTicket(ticket: string): CaptureTicketResult {
  const parts = ticket.split(".");
  if (parts.length !== 3) return { ok: false, reason: "malformed ticket" };
  const [version, body, mac] = parts as [string, string, string];
  if (version !== CAPTURE_TICKET_VERSION) return { ok: false, reason: "unknown ticket version" };

  let expected: string;
  try {
    expected = b64url(
      createHmac("sha256", captureKey()).update(`${version}.${body}`).digest(),
    );
  } catch {
    return { ok: false, reason: "capture key is not configured" };
  }

  // Compare in constant time, and only after a length check — `timingSafeEqual`
  // throws on a length mismatch rather than returning false.
  const a = Buffer.from(mac, "utf8");
  const b = Buffer.from(expected, "utf8");
  if (a.length !== b.length || !timingSafeEqual(a, b)) {
    return { ok: false, reason: "signature does not match" };
  }

  let claims: CaptureTicketClaims;
  try {
    claims = JSON.parse(Buffer.from(body, "base64url").toString("utf8")) as CaptureTicketClaims;
  } catch {
    return { ok: false, reason: "ticket payload is not readable" };
  }

  if (
    typeof claims?.sessionId !== "string" ||
    typeof claims?.orgId !== "string" ||
    typeof claims?.startUrl !== "string" ||
    typeof claims?.exp !== "number"
  ) {
    return { ok: false, reason: "ticket payload is incomplete" };
  }
  if (claims.exp * 1000 < Date.now()) return { ok: false, reason: "ticket has expired" };

  return { ok: true, claims };
}

/**
 * Ceilings a capture session cannot exceed.
 *
 * Every one of these exists because a capture session holds a real browser in
 * a container with finite memory, and the thing driving it is a person who may
 * simply close the tab. Without a cap an abandoned session pins a browser
 * forever, and enough abandoned sessions take the worker down — taking replay,
 * which is the part customers depend on, with it.
 */
export const CAPTURE_LIMITS = {
  /** Wall clock from first connect. A workflow demonstration is minutes, not hours. */
  maxSessionMs: 15 * 60 * 1000,
  /** No message from the client at all — the tab is gone, or the network is. */
  idleTimeoutMs: 2 * 60 * 1000,
  /**
   * Trace events retained. A runaway page (an animation loop firing `change`,
   * a redirect storm) must not grow the trace without bound in memory.
   */
  maxEvents: 5_000,
  /** Concurrent sessions per worker process, each holding its own browser. */
  maxConcurrentSessions: Number(process.env.GHOST_CAPTURE_MAX_SESSIONS ?? 3),
  /** Screencast frame size. Bigger costs bandwidth for no reviewing benefit. */
  frameWidth: 1280,
  frameHeight: 800,
  /**
   * How long a connected socket has to send its `auth` message before it is
   * dropped. An unauthenticated socket costs nothing but a file descriptor —
   * but only because it is never allowed to hold one for long.
   */
  authTimeoutMs: 5_000,
  /**
   * Bytes of unsent frame data past which new frames are dropped instead of
   * queued. A client on a slow link cannot be allowed to turn the worker's
   * memory into a frame backlog, and a stale frame has no value anyway — the
   * next one is 30ms behind it.
   */
  maxBufferedBytes: 4 * 1024 * 1024,
} as const;

/** Default viewport of the remote browser, and of the live view showing it. */
export const CAPTURE_VIEWPORT = { width: 1280, height: 800 } as const;

// ---------------------------------------------------------------------------
// Wire protocol
// ---------------------------------------------------------------------------

/**
 * What the user's browser may ask the remote one to do.
 *
 * Input is forwarded as coordinates because that is what a human moving a
 * mouse produces, and it never reaches the trace: the recorder inside the page
 * reads the accessible role and name off whatever the pointer landed on, and
 * the compiled step carries those. Coordinates drive the live session; they
 * are not, and must never become, automation identity.
 */
export type CaptureClientMessage =
  /**
   * First frame on the socket, always. The ticket travels in the body rather
   * than a query string so it never lands in an access log, a proxy trace, or
   * a `Referer` — and because the browser `WebSocket` API cannot set headers.
   */
  | { t: "auth"; ticket: string }
  | {
      t: "mouse";
      type: "mousePressed" | "mouseReleased" | "mouseMoved" | "mouseWheel";
      x: number;
      y: number;
      button?: "none" | "left" | "middle" | "right";
      clickCount?: number;
      deltaX?: number;
      deltaY?: number;
      modifiers?: number;
    }
  | {
      t: "key";
      type: "keyDown" | "keyUp" | "rawKeyDown" | "char";
      key?: string;
      code?: string;
      text?: string;
      modifiers?: number;
      windowsVirtualKeyCode?: number;
    }
  /** Bulk text (paste, IME commit) — cheaper and more reliable than key-by-key. */
  | { t: "text"; value: string }
  | { t: "navigate"; url: string }
  | { t: "back" }
  | { t: "forward" }
  | { t: "reload" }
  /** Finish: save the trace, compile it, and end the session. */
  | { t: "stop" }
  /** Abandon: throw the trace away and end the session. */
  | { t: "cancel" }
  /** Frame received — the worker sends the next only after this. */
  | { t: "ack" };

export type CaptureServerMessage =
  | { t: "ready"; url: string; width: number; height: number; deadline: number }
  /** A JPEG screencast frame, base64. Relayed live and never stored. */
  | { t: "frame"; data: string }
  | { t: "url"; url: string }
  /** How much the recorder has captured so far, so the user sees it working. */
  | { t: "events"; count: number }
  | {
      t: "stopped";
      recordingId: string;
      compileStatus: "READY" | "NONE";
      stepCount: number;
      notes: string[];
    }
  | { t: "canceled" }
  | { t: "error"; message: string };
