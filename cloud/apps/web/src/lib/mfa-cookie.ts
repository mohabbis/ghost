/**
 * The "this session has passed two-factor" marker.
 *
 * Deliberately *not* a claim on the Auth.js session token. Auth.js exposes a
 * client-callable session-update endpoint, so anything the `jwt` callback
 * accepts from an update is, in effect, client-settable — the wrong shape for
 * a flag whose entire job is to say a challenge was passed. This is a
 * separate httpOnly cookie the server mints only after verifying a code.
 *
 * Signed with `AUTH_SECRET` via **Web Crypto**, not `node:crypto`, so the same
 * implementation runs in the Edge middleware (which enforces it) and in the
 * Node route (which issues it). See `auth.config.ts` for why middleware
 * cannot pull in Node built-ins.
 *
 * Bound to the *sign-in*, not merely the account. Without that, a cookie
 * minted when the factor was proved would keep satisfying the gate across a
 * sign-out and a fresh sign-in for its whole lifetime — so someone holding
 * only the password could sign in on that machine and never be challenged,
 * which is the precise thing a second factor exists to prevent.
 *
 * Threat model, stated plainly: this defends against a *stolen or guessed
 * password*, which is what TOTP is for. It does not defend against an attacker
 * who has already stolen the browser's cookies — they would take this one
 * along with the session cookie.
 */

export const MFA_COOKIE_NAME = "ghost_mfa";

/** Matches the default session lifetime; a re-auth re-challenges anyway. */
const DEFAULT_TTL_SECONDS = 12 * 60 * 60;

interface Payload {
  uid: string;
  /** The sign-in this was issued for; see `signMfaCookie`. */
  sid: string;
  exp: number;
}

function b64urlEncode(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function b64urlDecode(value: string): Uint8Array<ArrayBuffer> {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(padded + "=".repeat((4 - (padded.length % 4)) % 4));
  const out = new Uint8Array(new ArrayBuffer(bin.length));
  for (let i = 0; i < bin.length; i += 1) out[i] = bin.charCodeAt(i);
  return out;
}

/** `TextEncoder` returns a `Uint8Array` TS treats as possibly SharedArrayBuffer-backed;
 *  Web Crypto requires an ArrayBuffer-backed view. */
function utf8(value: string): Uint8Array<ArrayBuffer> {
  const encoded = new TextEncoder().encode(value);
  const out = new Uint8Array(new ArrayBuffer(encoded.byteLength));
  out.set(encoded);
  return out;
}

async function hmacKey(secret: string): Promise<CryptoKey> {
  return crypto.subtle.importKey(
    "raw",
    utf8(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign", "verify"],
  );
}

function authSecret(): string {
  const secret = process.env.AUTH_SECRET;
  if (!secret) throw new Error("AUTH_SECRET is required to sign the MFA cookie");
  return secret;
}

export async function signMfaCookie(
  userId: string,
  sessionId: string,
  ttlSeconds: number = DEFAULT_TTL_SECONDS,
): Promise<{ value: string; maxAge: number }> {
  const payload: Payload = {
    uid: userId,
    sid: sessionId,
    exp: Math.floor(Date.now() / 1000) + ttlSeconds,
  };
  const body = b64urlEncode(utf8(JSON.stringify(payload)));
  const sig = await crypto.subtle.sign("HMAC", await hmacKey(authSecret()), utf8(body));
  return { value: `${body}.${b64urlEncode(new Uint8Array(sig))}`, maxAge: ttlSeconds };
}

/**
 * True only for a cookie that is well-formed, correctly signed, unexpired,
 * and issued to this exact user. Any failure — including a malformed value —
 * is a plain `false`; nothing here throws into the middleware.
 */
export async function verifyMfaCookie(
  value: string | undefined,
  userId: string,
  sessionId: string | undefined,
): Promise<boolean> {
  if (!value) return false;
  const dot = value.lastIndexOf(".");
  if (dot <= 0) return false;
  const body = value.slice(0, dot);
  const sig = value.slice(dot + 1);

  try {
    const ok = await crypto.subtle.verify(
      "HMAC",
      await hmacKey(authSecret()),
      b64urlDecode(sig),
      utf8(body),
    );
    if (!ok) return false;

    const payload = JSON.parse(new TextDecoder().decode(b64urlDecode(body))) as Payload;
    if (payload.uid !== userId) return false;
    // Bound to the sign-in, not just the account: a cookie minted before a
    // sign-out must not satisfy the gate for the next sign-in.
    if (!sessionId || payload.sid !== sessionId) return false;
    return payload.exp > Math.floor(Date.now() / 1000);
  } catch {
    return false;
  }
}
