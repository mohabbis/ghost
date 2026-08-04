import { createCipheriv, createDecipheriv, randomBytes } from "node:crypto";

/**
 * Envelope encryption for stored TOTP secrets.
 *
 * Same AES-256-GCM construction as `secretbox.ts`, but under its own key.
 * They cannot share one: `GHOST_SESSION_KEY` is deliberately absent from the
 * web app's environment so a web-side vulnerability cannot decrypt a captured
 * browser session, and verifying a TOTP code is something the web app must do
 * on every sign-in. Giving the web app the session key to reuse this code
 * would trade away that separation for a saved environment variable.
 *
 * Why encrypt at all, when the key usually lives beside the database
 * credentials: a TOTP secret is a *shared* secret, so a database dump alone
 * otherwise lets an attacker generate valid codes indefinitely, silently, for
 * every user. Requiring the attacker to also reach the process environment is
 * a real step up, and it is cheap.
 *
 * Fails closed. With no key configured, enrolment is refused rather than
 * falling back to storing the secret in the clear.
 */

const IV_BYTES = 12;
const TAG_BYTES = 16;
const KEY_BYTES = 32;

export class MissingMfaKeyError extends Error {
  constructor() {
    super(
      "GHOST_MFA_KEY is not set. Generate one with `openssl rand -base64 32`. " +
        "It is required to store two-factor secrets; Ghost will not keep them unencrypted.",
    );
    this.name = "MissingMfaKeyError";
  }
}

function key(): Buffer {
  const raw = process.env.GHOST_MFA_KEY;
  if (!raw) throw new MissingMfaKeyError();
  const buf = Buffer.from(raw, "base64");
  if (buf.length !== KEY_BYTES) {
    throw new Error(
      `GHOST_MFA_KEY must decode to exactly ${KEY_BYTES} bytes (got ${buf.length}). ` +
        "Generate one with `openssl rand -base64 32`.",
    );
  }
  return buf;
}

/** Whether MFA can be offered at all in this deployment. */
export function mfaKeyConfigured(): boolean {
  try {
    key();
    return true;
  } catch {
    return false;
  }
}

/**
 * `userId` is bound in as associated data, so a ciphertext lifted from one
 * user's row cannot be pasted into another's to transplant a known secret —
 * it fails authentication exactly as a tampered ciphertext would.
 */
export function sealMfaSecret(secret: string, userId: string): string {
  const iv = randomBytes(IV_BYTES);
  const cipher = createCipheriv("aes-256-gcm", key(), iv);
  cipher.setAAD(Buffer.from(userId, "utf8"));
  const ciphertext = Buffer.concat([cipher.update(secret, "utf8"), cipher.final()]);
  return Buffer.concat([iv, cipher.getAuthTag(), ciphertext]).toString("base64");
}

export function openMfaSecret(sealed: string, userId: string): string {
  const buf = Buffer.from(sealed, "base64");
  if (buf.length < IV_BYTES + TAG_BYTES) throw new Error("sealed MFA secret is truncated");
  const iv = buf.subarray(0, IV_BYTES);
  const tag = buf.subarray(IV_BYTES, IV_BYTES + TAG_BYTES);
  const ciphertext = buf.subarray(IV_BYTES + TAG_BYTES);

  const decipher = createDecipheriv("aes-256-gcm", key(), iv);
  decipher.setAAD(Buffer.from(userId, "utf8"));
  decipher.setAuthTag(tag);
  return Buffer.concat([decipher.update(ciphertext), decipher.final()]).toString("utf8");
}
