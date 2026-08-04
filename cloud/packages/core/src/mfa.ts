import { createHmac, randomBytes, timingSafeEqual, createHash } from "node:crypto";

/**
 * Time-based one-time passwords (RFC 6238 over RFC 4226 HOTP).
 *
 * Implemented directly rather than pulled from a package: it is roughly sixty
 * lines of well-specified arithmetic, both RFCs publish test vectors, and this
 * sits on the authentication path where an unaudited transitive dependency is
 * a poor trade. `mfa.test.ts` checks it against the published vectors for both
 * RFCs, which is a stronger correctness argument than "a library did it".
 *
 * SHA-1 is not a mistake here. RFC 6238 specifies HMAC-SHA1 with a 30-second
 * step, and every authenticator app (Google, 1Password, Authy, Aegis) assumes
 * it. HMAC-SHA1 is not affected by SHA-1's collision weaknesses, which concern
 * collision resistance rather than the PRF property HMAC relies on.
 */

const STEP_SECONDS = 30;
const DEFAULT_DIGITS = 6;
/** Accept the neighbouring steps: phone clocks drift, and people type slowly. */
const DEFAULT_SKEW_STEPS = 1;

// RFC 4648 base32, the alphabet every authenticator app expects.
const BASE32_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

export function base32Encode(buf: Buffer): string {
  let bits = 0;
  let value = 0;
  let out = "";
  for (const byte of buf) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      out += BASE32_ALPHABET[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) out += BASE32_ALPHABET[(value << (5 - bits)) & 31];
  return out;
}

export function base32Decode(input: string): Buffer {
  // Tolerate the spacing and padding people paste in from a screen.
  const cleaned = input.toUpperCase().replace(/[\s=-]/g, "");
  let bits = 0;
  let value = 0;
  const out: number[] = [];
  for (const char of cleaned) {
    const idx = BASE32_ALPHABET.indexOf(char);
    if (idx === -1) throw new Error(`invalid base32 character: ${char}`);
    value = (value << 5) | idx;
    bits += 5;
    if (bits >= 8) {
      out.push((value >>> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  return Buffer.from(out);
}

/** A fresh 160-bit secret — the size RFC 4226 recommends for HMAC-SHA1. */
export function generateTotpSecret(): string {
  return base32Encode(randomBytes(20));
}

/** RFC 4226 HOTP. Exposed mainly so the published vectors can be tested. */
export function hotp(secret: Buffer, counter: number, digits = DEFAULT_DIGITS): string {
  const counterBuf = Buffer.alloc(8);
  // Counter is a 64-bit big-endian integer. Written as two 32-bit halves
  // because `writeBigUInt64BE` would force every caller to deal in BigInt for
  // values that never approach 2^53 in practice.
  counterBuf.writeUInt32BE(Math.floor(counter / 0x100000000), 0);
  counterBuf.writeUInt32BE(counter >>> 0, 4);

  const digest = createHmac("sha1", secret).update(counterBuf).digest();

  // Dynamic truncation (RFC 4226 §5.3): the low nibble of the last byte picks
  // the offset; the high bit of the selected word is masked off so the result
  // is sign-independent across implementations.
  const offset = digest[digest.length - 1]! & 0x0f;
  const binary =
    ((digest[offset]! & 0x7f) << 24) |
    ((digest[offset + 1]! & 0xff) << 16) |
    ((digest[offset + 2]! & 0xff) << 8) |
    (digest[offset + 3]! & 0xff);

  return (binary % 10 ** digits).toString().padStart(digits, "0");
}

export function totp(
  secretBase32: string,
  atMs: number = Date.now(),
  digits = DEFAULT_DIGITS,
): string {
  const counter = Math.floor(atMs / 1000 / STEP_SECONDS);
  return hotp(base32Decode(secretBase32), counter, digits);
}

/**
 * Verify a submitted code, allowing one step of clock skew either way.
 *
 * Comparison is constant-time: a timing oracle on a six-digit code is a small
 * search space, and this runs on the authentication path.
 */
export function verifyTotp(
  secretBase32: string,
  code: string,
  opts: { atMs?: number; digits?: number; skewSteps?: number } = {},
): boolean {
  const digits = opts.digits ?? DEFAULT_DIGITS;
  const skew = opts.skewSteps ?? DEFAULT_SKEW_STEPS;
  const submitted = code.replace(/\s/g, "");
  if (!/^\d+$/.test(submitted) || submitted.length !== digits) return false;

  const secret = base32Decode(secretBase32);
  const counter = Math.floor((opts.atMs ?? Date.now()) / 1000 / STEP_SECONDS);

  let matched = false;
  for (let i = -skew; i <= skew; i += 1) {
    const candidate = hotp(secret, counter + i, digits);
    // No early exit: checking every window regardless keeps the work constant
    // whether the match is in the first window or the last.
    const a = Buffer.from(candidate, "utf8");
    const b = Buffer.from(submitted, "utf8");
    if (a.length === b.length && timingSafeEqual(a, b)) matched = true;
  }
  return matched;
}

/**
 * The `otpauth://` URI an authenticator app consumes, normally via QR code.
 *
 * `issuer` appears both as a label prefix and a parameter — apps disagree on
 * which they read, and supplying only one produces entries labelled
 * "Unknown" in some of them.
 */
export function totpUri(input: { secret: string; account: string; issuer?: string }): string {
  const issuer = input.issuer ?? "Ghost";
  const label = `${encodeURIComponent(issuer)}:${encodeURIComponent(input.account)}`;
  const params = new URLSearchParams({
    secret: input.secret,
    issuer,
    algorithm: "SHA1",
    digits: String(DEFAULT_DIGITS),
    period: String(STEP_SECONDS),
  });
  return `otpauth://totp/${label}?${params.toString()}`;
}

// --- Recovery codes --------------------------------------------------------

const RECOVERY_CODE_COUNT = 10;

/**
 * Single-use codes for when the authenticator is lost.
 *
 * Stored as SHA-256 digests, like every other credential here, so the database
 * alone never yields a usable code. Plain SHA-256 rather than a slow KDF is
 * appropriate *because* these are high-entropy (80 bits) random values, not
 * user-chosen passwords — there is no dictionary to run.
 */
export function generateRecoveryCodes(count = RECOVERY_CODE_COUNT): string[] {
  return Array.from({ length: count }, () => {
    const raw = base32Encode(randomBytes(10)).slice(0, 16).toLowerCase();
    return `${raw.slice(0, 4)}-${raw.slice(4, 8)}-${raw.slice(8, 12)}-${raw.slice(12, 16)}`;
  });
}

export function normalizeRecoveryCode(code: string): string {
  return code.trim().toLowerCase().replace(/\s/g, "");
}

export function hashRecoveryCode(code: string): string {
  return createHash("sha256").update(normalizeRecoveryCode(code), "utf8").digest("hex");
}
