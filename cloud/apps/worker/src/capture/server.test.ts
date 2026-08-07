import { afterAll, beforeAll, describe, expect, it } from "vitest";
import WebSocket from "ws";
import { mintCaptureTicket } from "@ghost/core/recording/capture";
import type { CaptureServerMessage } from "@ghost/core/recording/capture";
import { startCaptureServer, type CaptureServer } from "./server.js";
import { normalizeUrl } from "./session.js";

/**
 * The socket's refusal paths.
 *
 * Every case here ends without a browser being launched, which is the point:
 * the worker must decide whether a connection is allowed *before* it spends a
 * Chromium on it. A test that got as far as a real session would be testing
 * something else.
 */

const KEY = "test-capture-key-not-a-secret-0123456789";
const PORT = 18787;

let server: CaptureServer | null = null;

beforeAll(() => {
  process.env.GHOST_CAPTURE_KEY = KEY;
  process.env.GHOST_CAPTURE_PORT = String(PORT);
  server = startCaptureServer();
});

afterAll(async () => {
  await server?.close();
  delete process.env.GHOST_CAPTURE_KEY;
  delete process.env.GHOST_CAPTURE_PORT;
});

/** Connects, sends, and resolves with the first server message. */
function firstMessage(send: unknown): Promise<CaptureServerMessage> {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(`ws://127.0.0.1:${PORT}/capture`);
    const timer = setTimeout(() => {
      socket.close();
      reject(new Error("no reply from the capture server"));
    }, 5_000);
    socket.on("open", () => socket.send(JSON.stringify(send)));
    socket.on("message", (raw) => {
      clearTimeout(timer);
      socket.close();
      resolve(JSON.parse(String(raw)) as CaptureServerMessage);
    });
    socket.on("error", (err) => {
      clearTimeout(timer);
      reject(err);
    });
  });
}

describe("capture server", () => {
  it("listens when a key is configured", () => {
    expect(server).not.toBeNull();
  });

  it("reports health without a session", async () => {
    const res = await fetch(`http://127.0.0.1:${PORT}/health`);
    expect(res.ok).toBe(true);
    expect(await res.json()).toMatchObject({ ok: true, sessions: 0 });
  });

  it("refuses input from a socket that has not authenticated", async () => {
    // The case that matters most: no ticket, straight to driving a browser.
    // There must be no browser to drive.
    const reply = await firstMessage({ t: "mouse", type: "mousePressed", x: 10, y: 10 });
    expect(reply.t).toBe("error");
    if (reply.t !== "error") return;
    expect(reply.message).toMatch(/auth ticket/i);
  });

  it("refuses a ticket signed with the wrong key", async () => {
    process.env.GHOST_CAPTURE_KEY = "some-other-key-entirely-000000000000000";
    const forged = mintCaptureTicket({
      sessionId: "s",
      orgId: "org-victim",
      userId: null,
      startUrl: "https://example.com",
    });
    process.env.GHOST_CAPTURE_KEY = KEY;

    const reply = await firstMessage({ t: "auth", ticket: forged });
    expect(reply.t).toBe("error");
  });

  it("refuses an expired ticket", async () => {
    const stale = mintCaptureTicket({
      sessionId: "s",
      orgId: "org-1",
      userId: null,
      startUrl: "https://example.com",
      exp: Math.floor(Date.now() / 1000) - 60,
    });
    const reply = await firstMessage({ t: "auth", ticket: stale });
    expect(reply.t).toBe("error");
  });

  it("refuses unreadable input", async () => {
    const reply = await new Promise<CaptureServerMessage>((resolve, reject) => {
      const socket = new WebSocket(`ws://127.0.0.1:${PORT}/capture`);
      socket.on("open", () => socket.send("not json"));
      socket.on("message", (raw) => {
        socket.close();
        resolve(JSON.parse(String(raw)) as CaptureServerMessage);
      });
      socket.on("error", reject);
    });
    expect(reply.t).toBe("error");
  });
});

describe("startCaptureServer without a key", () => {
  it("does not listen at all", () => {
    // Off means off: no port, rather than a port that accepts anyone. The
    // feature switch and the authentication are the same thing on purpose.
    const key = process.env.GHOST_CAPTURE_KEY;
    delete process.env.GHOST_CAPTURE_KEY;
    expect(startCaptureServer()).toBeNull();
    process.env.GHOST_CAPTURE_KEY = key;
  });
});

describe("normalizeUrl", () => {
  it("accepts what a person types into an address bar", () => {
    expect(normalizeUrl("example.com")).toBe("https://example.com");
    expect(normalizeUrl("  example.com  ")).toBe("https://example.com");
  });

  it("leaves an explicit scheme alone", () => {
    expect(normalizeUrl("http://example.com/a")).toBe("http://example.com/a");
    expect(normalizeUrl("https://example.com/a")).toBe("https://example.com/a");
  });
});
