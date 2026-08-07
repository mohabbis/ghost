import { auth } from "@/auth";
import {
  CAPTURE_TICKET_TTL_SECONDS,
  captureConfigured,
  mintCaptureTicket,
  newCaptureSessionId,
} from "@ghost/core/recording/capture";

/**
 * Open a remote capture session: a browser in the worker that the user drives
 * to demonstrate a workflow.
 *
 * This route does not start anything. It authenticates the caller — the one
 * thing the worker cannot do, having no session table — and hands back a
 * short-lived signed ticket plus the socket to present it on. The browser is
 * launched by the worker when that socket connects, so a request that is
 * abandoned in the two minutes before then costs nothing.
 *
 * No `Recording` row is created here either. It is created by `ingestTrace`
 * when a trace exists, which is the only moment at which a recording is a real
 * thing; a user who opens this page and closes it leaves nothing behind to
 * explain.
 *
 * Touches: authentication (session), no database, no network.
 */

/** Where a capture opens when the user gives no URL of their own. */
const DEFAULT_START_URL = "about:blank";

export async function POST(req: Request) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }

  const socketUrl = process.env.NEXT_PUBLIC_GHOST_CAPTURE_URL;
  if (!captureConfigured() || !socketUrl) {
    // Off, not broken. Said plainly so the operator knows which of the two
    // variables to set rather than reading it as a bug.
    return Response.json(
      {
        error:
          "Recording in a cloud browser is not enabled on this deployment. It needs " +
          "GHOST_CAPTURE_KEY (the same value on the web app and the worker) and " +
          "NEXT_PUBLIC_GHOST_CAPTURE_URL pointing at the worker's capture socket.",
      },
      { status: 501 },
    );
  }

  const body = (await req.json().catch(() => ({}))) as { startUrl?: unknown };
  const startUrl = normalizeStartUrl(body.startUrl);
  if (!startUrl) {
    return Response.json(
      { error: "startUrl must be an http or https address" },
      { status: 400 },
    );
  }

  const sessionId = newCaptureSessionId();
  // `startUrl` is signed into the ticket rather than sent alongside it, so the
  // page the browser opens on is the one this authenticated request asked for
  // and not something swapped in on the way to the worker.
  const ticket = mintCaptureTicket({
    sessionId,
    orgId: session.user.orgId,
    userId: session.user.id ?? null,
    startUrl,
  });

  return Response.json({
    sessionId,
    ticket,
    socketUrl,
    startUrl,
    expiresInSeconds: CAPTURE_TICKET_TTL_SECONDS,
  });
}

/**
 * Only `http`/`https`, and nothing scheme-less.
 *
 * The worker opens this in a real browser, so `file:` would read the
 * container's disk and `javascript:` would run in whatever page is loaded.
 * Neither is a workflow anyone is demonstrating.
 */
function normalizeStartUrl(input: unknown): string | null {
  if (input === undefined || input === null || input === "") return DEFAULT_START_URL;
  if (typeof input !== "string") return null;
  const trimmed = input.trim();
  if (!trimmed) return DEFAULT_START_URL;

  const candidate = /^[a-z][a-z0-9+.-]*:/i.test(trimmed) ? trimmed : `https://${trimmed}`;
  let parsed: URL;
  try {
    parsed = new URL(candidate);
  } catch {
    return null;
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return null;
  return parsed.toString();
}
