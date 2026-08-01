import { createCipheriv, createDecipheriv, randomBytes, createHash } from "node:crypto";

/**
 * Authenticated encryption for at-rest blobs the engine must store but nobody
 * should be able to read from storage alone.
 *
 * The motivating case is Playwright's `storageState`: cookies and localStorage
 * captured when a run halts at an approval gate. Those cookies are live bearer
 * credentials for the customer's bank, CRM, or payment provider. A screenshot
 * leaking is embarrassing; a session blob leaking is account takeover.
 *
 * AES-256-GCM. Layout: `IV (12) || TAG (16) || CIPHERTEXT`.
 *
 * Callers pass **associated data** binding the ciphertext to where it belongs —
 * for session blobs, the run id and artifact key. Without it, encryption alone
 * proves only that *some* blob sealed under the worker key is authentic, not
 * that it is the right one: every run shares that key, so a blob swapped
 * between two runs would decrypt cleanly and resume a workflow under another
 * customer's browser session. AAD is authenticated but not encrypted, and a
 * mismatch fails the same way a tampered ciphertext does.
 *
 * The key is read from `GHOST_SESSION_KEY` and is expected to be present only
 * in the **worker's** environment. The web app never holds it, so a web-side
 * vulnerability with filesystem or bucket read access still cannot decrypt a
 * session blob.
 */

const IV_BYTES = 12;
const TAG_BYTES = 16;
const KEY_BYTES = 32;

export class MissingSessionKeyError extends Error {
  constructor() {
    super(
      "GHOST_SESSION_KEY is not set. Generate one with `openssl rand -base64 32`. " +
        "It is required to capture browser session state across an approval gate.",
    );
    this.name = "MissingSessionKeyError";
  }
}

export function seal(plaintext: Buffer, key: Buffer, aad?: string): Buffer {
  if (key.length !== KEY_BYTES) {
    throw new Error(`session key must be ${KEY_BYTES} bytes, got ${key.length}`);
  }
  // A fresh random IV per seal. Reusing an IV under GCM is catastrophic, so this
  // is never derived from the plaintext or a counter.
  const iv = randomBytes(IV_BYTES);
  const cipher = createCipheriv("aes-256-gcm", key, iv);
  if (aad !== undefined) cipher.setAAD(Buffer.from(aad, "utf8"));
  const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()]);
  return Buffer.concat([iv, cipher.getAuthTag(), ciphertext]);
}

export function open(sealed: Buffer, key: Buffer, aad?: string): Buffer {
  if (key.length !== KEY_BYTES) {
    throw new Error(`session key must be ${KEY_BYTES} bytes, got ${key.length}`);
  }
  if (sealed.length < IV_BYTES + TAG_BYTES) {
    throw new Error("sealed blob is too short to be valid");
  }
  const iv = sealed.subarray(0, IV_BYTES);
  const tag = sealed.subarray(IV_BYTES, IV_BYTES + TAG_BYTES);
  const ciphertext = sealed.subarray(IV_BYTES + TAG_BYTES);

  const decipher = createDecipheriv("aes-256-gcm", key, iv);
  decipher.setAuthTag(tag);
  if (aad !== undefined) decipher.setAAD(Buffer.from(aad, "utf8"));
  // Throws on a tampered blob, a wrong key, or AAD that does not match what was
  // sealed — GCM authenticates before we ever hand the bytes back.
  return Buffer.concat([decipher.update(ciphertext), decipher.final()]);
}

/**
 * The associated data for a run's captured browser session.
 *
 * Binds the ciphertext to both the run and the exact artifact key it was
 * written to, so a blob cannot be replayed into a different run or a different
 * gate of the same run.
 */
export function sessionAad(runId: string, artifactKey: string): string {
  return `ghost:session:v1:${runId}:${artifactKey}`;
}

/** Read and validate the worker's session key. Throws if absent or malformed. */
export function sessionKeyFromEnv(): Buffer {
  const raw = process.env.GHOST_SESSION_KEY;
  if (!raw) throw new MissingSessionKeyError();
  const key = Buffer.from(raw, "base64");
  if (key.length !== KEY_BYTES) {
    throw new Error(
      `GHOST_SESSION_KEY must decode to ${KEY_BYTES} bytes (got ${key.length}). ` +
        "Generate one with `openssl rand -base64 32`.",
    );
  }
  return key;
}

/** True when a session key is configured, without throwing. */
export function hasSessionKey(): boolean {
  try {
    sessionKeyFromEnv();
    return true;
  } catch {
    return false;
  }
}

/** Digest of a sealed blob, for committing to it in the audit chain. */
export function digest(buf: Buffer): string {
  return createHash("sha256").update(buf).digest("hex");
}
