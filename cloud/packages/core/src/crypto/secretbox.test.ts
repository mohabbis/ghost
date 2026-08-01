import { describe, expect, it } from "vitest";
import { randomBytes } from "node:crypto";
import {
  seal,
  open,
  digest,
  sessionAad,
  sessionKeyFromEnv,
  hasSessionKey,
} from "./secretbox.js";

const key = randomBytes(32);

describe("secretbox", () => {
  it("round-trips a payload", () => {
    const plaintext = Buffer.from(JSON.stringify({ cookies: [{ name: "sid", value: "abc" }] }));
    expect(open(seal(plaintext, key), key)).toEqual(plaintext);
  });

  it("does not leak plaintext into the sealed blob", () => {
    const sealed = seal(Buffer.from("super-secret-session-token"), key);
    expect(sealed.toString("utf8")).not.toContain("super-secret-session-token");
  });

  it("uses a fresh IV for identical plaintexts", () => {
    // Reusing an IV under GCM is catastrophic, so two seals of the same bytes
    // must not produce the same ciphertext.
    const plaintext = Buffer.from("same every time");
    expect(seal(plaintext, key).equals(seal(plaintext, key))).toBe(false);
  });

  it("rejects a tampered blob", () => {
    const sealed = seal(Buffer.from("payload"), key);
    sealed.writeUInt8(sealed.readUInt8(sealed.length - 1) ^ 0xff, sealed.length - 1);
    expect(() => open(sealed, key)).toThrow();
  });

  it("rejects a wrong key", () => {
    const sealed = seal(Buffer.from("payload"), key);
    expect(() => open(sealed, randomBytes(32))).toThrow();
  });

  it("rejects a malformed key length", () => {
    expect(() => seal(Buffer.from("x"), randomBytes(16))).toThrow(/32 bytes/);
  });

  it("rejects a truncated blob", () => {
    expect(() => open(Buffer.alloc(4), key)).toThrow(/too short/);
  });

  it("reads a base64 key from the environment", () => {
    const prev = process.env.GHOST_SESSION_KEY;
    try {
      process.env.GHOST_SESSION_KEY = key.toString("base64");
      expect(sessionKeyFromEnv()).toEqual(key);
      expect(hasSessionKey()).toBe(true);

      process.env.GHOST_SESSION_KEY = "too-short";
      expect(() => sessionKeyFromEnv()).toThrow(/must decode to 32 bytes/);
      expect(hasSessionKey()).toBe(false);

      delete process.env.GHOST_SESSION_KEY;
      expect(() => sessionKeyFromEnv()).toThrow(/GHOST_SESSION_KEY is not set/);
    } finally {
      if (prev === undefined) delete process.env.GHOST_SESSION_KEY;
      else process.env.GHOST_SESSION_KEY = prev;
    }
  });

  // --- Associated data binds a blob to where it belongs --------------------

  it("round-trips with matching associated data", () => {
    const aad = sessionAad("run_1", "runs/run_1/session/gate-2.bin");
    const plaintext = Buffer.from("cookies");
    expect(open(seal(plaintext, key, aad), key, aad)).toEqual(plaintext);
  });

  it("refuses a blob sealed for a different run", () => {
    // The attack this closes: every run shares one worker key, so without AAD a
    // blob lifted from another run decrypts cleanly and would resume a workflow
    // under someone else's browser session.
    const sealed = seal(
      Buffer.from("victim session"),
      key,
      sessionAad("run_victim", "runs/run_victim/session/gate-0.bin"),
    );
    expect(() =>
      open(sealed, key, sessionAad("run_attacker", "runs/run_attacker/session/gate-0.bin")),
    ).toThrow();
  });

  it("refuses a blob sealed for a different gate of the same run", () => {
    const sealed = seal(Buffer.from("s"), key, sessionAad("run_1", "runs/run_1/session/gate-0.bin"));
    expect(() =>
      open(sealed, key, sessionAad("run_1", "runs/run_1/session/gate-9.bin")),
    ).toThrow();
  });

  it("refuses an AAD-sealed blob opened without AAD, and vice versa", () => {
    const aad = sessionAad("run_1", "k");
    expect(() => open(seal(Buffer.from("s"), key, aad), key)).toThrow();
    expect(() => open(seal(Buffer.from("s"), key), key, aad)).toThrow();
  });

  it("digests deterministically", () => {
    const buf = Buffer.from("abc");
    expect(digest(buf)).toBe(digest(Buffer.from("abc")));
    expect(digest(buf)).toHaveLength(64);
  });
});
