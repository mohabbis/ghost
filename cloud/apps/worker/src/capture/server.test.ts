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

let server: CaptureServer;

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
  it("mounts capture when a key is configured", () => {
    expect(server.captureEnabled).toBe(true);
  });

  it("reports health without a session", async () => {
    const res = await fetch(`http://127.0.0.1:${PORT}/health`);
    expect(res.ok).toBe(true);
    expect(await res.json()).toMatchObject({ ok: true, capture: true, sessions: 0 });
  });

  it("answers the root path too", async () => {
    // A host's default health check probes `/`. Answering only `/health`
    // means the platform reports the worker unhealthy and restarts it in a
    // loop, which reads as a crashing worker rather than a missing setting.
    const res = await fetch(`http://127.0.0.1:${PORT}/`);
    expect(res.ok).toBe(true);
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
  it("still binds a port, but mounts no capture endpoint", async () => {
    // Two separate things, deliberately. The port must exist or a host that
    // runs this as a web service — the only kind that can accept an inbound
    // socket — fails the deploy and takes replay down with it. The capture
    // endpoint must NOT exist, because a reachable one with no key to check
    // is a browser anyone can drive.
    const key = process.env.GHOST_CAPTURE_KEY;
    delete process.env.GHOST_CAPTURE_KEY;
    process.env.GHOST_CAPTURE_PORT = String(PORT + 1);

    const off = startCaptureServer();
    try {
      expect(off.captureEnabled).toBe(false);
      const res = await fetch(`http://127.0.0.1:${PORT + 1}/health`);
      expect(await res.json()).toMatchObject({ ok: true, capture: false });

      // `/capture` is not routed, so the upgrade is refused outright.
      await expect(
        new Promise((resolve, reject) => {
          const socket = new WebSocket(`ws://127.0.0.1:${PORT + 1}/capture`);
          socket.on("open", () => resolve("connected"));
          socket.on("error", reject);
        }),
      ).rejects.toThrow();
    } finally {
      await off.close();
      process.env.GHOST_CAPTURE_KEY = key;
      process.env.GHOST_CAPTURE_PORT = String(PORT);
    }
  });
});

describe("port selection", () => {
  it("falls back to the host-injected PORT", async () => {
    // What lets one existing service host both the queue consumer and the
    // capture socket: Render, Railway and Fly all inject `PORT`.
    const configured = process.env.GHOST_CAPTURE_PORT;
    delete process.env.GHOST_CAPTURE_PORT;
    process.env.PORT = String(PORT + 2);

    const injected = startCaptureServer();
    try {
      expect(injected.port).toBe(PORT + 2);
    } finally {
      await injected.close();
      delete process.env.PORT;
      process.env.GHOST_CAPTURE_PORT = configured;
    }
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
