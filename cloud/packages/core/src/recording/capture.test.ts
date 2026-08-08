import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  CAPTURE_TICKET_VERSION,
  captureConfigured,
  mintCaptureTicket,
  newCaptureSessionId,
  verifyCaptureTicket,
} from "./capture.js";

/**
 * The capture ticket is the whole of the worker's authentication. It has no
 * session table to fall back on and never calls the web app, so anything this
 * misses is a browser someone else gets to drive.
 */

const KEY = "test-capture-key-not-a-secret-0123456789";

function claims(over: Partial<Parameters<typeof mintCaptureTicket>[0]> = {}) {
  return {
    sessionId: "sess-1",
    orgId: "org-1",
    userId: "user-1",
    startUrl: "https://example.com/inbox",
    ...over,
  };
}

describe("capture tickets", () => {
  beforeEach(() => {
    process.env.GHOST_CAPTURE_KEY = KEY;
  });
  afterEach(() => {
    delete process.env.GHOST_CAPTURE_KEY;
  });

  it("round-trips the claims the worker acts on", () => {
    const result = verifyCaptureTicket(mintCaptureTicket(claims()));
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.claims.sessionId).toBe("sess-1");
    expect(result.claims.orgId).toBe("org-1");
    expect(result.claims.userId).toBe("user-1");
    expect(result.claims.startUrl).toBe("https://example.com/inbox");
  });

  it("refuses a ticket signed with a different key", () => {
    const ticket = mintCaptureTicket(claims());
    process.env.GHOST_CAPTURE_KEY = "a-completely-different-key-000000000000";
    expect(verifyCaptureTicket(ticket)).toMatchObject({ ok: false });
  });

  it("refuses a ticket whose claims were edited", () => {
    // The attack the signature exists for: take your own valid ticket and
    // point it at someone else's organization.
    const ticket = mintCaptureTicket(claims());
    const [version, body, mac] = ticket.split(".") as [string, string, string];
    const payload = JSON.parse(Buffer.from(body, "base64url").toString("utf8"));
    payload.orgId = "org-someone-else";
    const forged = Buffer.from(JSON.stringify(payload), "utf8").toString("base64url");

    const result = verifyCaptureTicket(`${version}.${forged}.${mac}`);
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.reason).toMatch(/signature/);
  });

  it("refuses an expired ticket", () => {
    const ticket = mintCaptureTicket({ ...claims(), exp: Math.floor(Date.now() / 1000) - 1 });
    const result = verifyCaptureTicket(ticket);
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.reason).toMatch(/expired/);
  });

  it("refuses malformed input rather than throwing", () => {
    for (const bad of ["", "nonsense", "a.b", "a.b.c.d", `${CAPTURE_TICKET_VERSION}.!!.!!`]) {
      expect(verifyCaptureTicket(bad).ok).toBe(false);
    }
  });

  it("refuses a ticket from an unknown version", () => {
    const ticket = mintCaptureTicket(claims()).replace(CAPTURE_TICKET_VERSION, "gcap9");
    expect(verifyCaptureTicket(ticket)).toMatchObject({ ok: false });
  });

  it("is off, not open, when no key is configured", () => {
    delete process.env.GHOST_CAPTURE_KEY;
    expect(captureConfigured()).toBe(false);
    // Nothing verifies against an absent key — including a ticket that was
    // valid a moment ago.
    process.env.GHOST_CAPTURE_KEY = KEY;
    const ticket = mintCaptureTicket(claims());
    delete process.env.GHOST_CAPTURE_KEY;
    expect(verifyCaptureTicket(ticket).ok).toBe(false);
  });

  it("gives every session its own id", () => {
    expect(newCaptureSessionId()).not.toBe(newCaptureSessionId());
  });
});
