/**
 * The capture wire contract: what the two sides of a session say to each other.
 *
 * Separate from `capture.ts` because this half runs in the *user's* browser.
 * That file signs and verifies tickets and therefore imports `node:crypto`,
 * which a client bundle cannot resolve — the live-view component importing one
 * value from it was enough to fail the production build. The split is not
 * bookkeeping: ticket signing is a server secret, and the message shapes are
 * shared by definition.
 *
 * Nothing here may import a Node builtin, read `process.env`, or touch a
 * database. It is types and constants both sides agree on.
 */

/** Default viewport of the remote browser, and of the live view showing it. */
export const CAPTURE_VIEWPORT = { width: 1280, height: 800 } as const;

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
