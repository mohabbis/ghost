/**
 * Reject URLs that would let a workflow reach private network targets from
 * the worker's browser.
 *
 * `navigate` previously accepted any `z.string().url()`. The worker launches
 * Chromium with `--no-sandbox`, so a crafted step could hit cloud metadata
 * endpoints (or RFC1918 hosts on the worker's network) and `extract` the
 * response back out — credential theft by any authenticated author.
 *
 * Policy:
 *   1. Only `http:` / `https:`.
 *   2. Always refuse cloud-metadata hostnames and `169.254.169.254`.
 *   3. Loopback (`localhost`, `127.0.0.0/8`, `::1`) is allowed — the local
 *      demo fixture and the worker's hermetic test servers live there.
 *   4. Other private / link-local / ULA hosts are refused, **except** when
 *      the host matches `APP_URL` (a self-hosted Ghost on a private IP).
 *
 * DNS rebinding is outside this check — that needs a resolve-then-connect
 * gate in the browser launcher.
 */

const METADATA_HOSTNAMES = new Set([
  "metadata.google.internal",
  "metadata.google",
  "metadata.azure.com",
  "metadata",
]);

/** IPv4 dotted-quad → 32-bit int, or null if not a literal IPv4. */
function parseIpv4(host: string): number | null {
  const m = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(host);
  if (!m) return null;
  const octets = m.slice(1).map(Number);
  if (octets.some((n) => n > 255)) return null;
  return ((octets[0]! << 24) | (octets[1]! << 16) | (octets[2]! << 8) | octets[3]!) >>> 0;
}

function isLoopbackHost(host: string): boolean {
  if (host === "localhost" || host.endsWith(".localhost")) return true;
  if (host === "::1") return true;
  const ipv4 = parseIpv4(host);
  // 127.0.0.0/8
  if (ipv4 !== null && (ipv4 & 0xff000000) === 0x7f000000) return true;
  return false;
}

function isPrivateNonLoopback(host: string): boolean {
  if (host.endsWith(".local")) return true;
  const ipv4 = parseIpv4(host);
  if (ipv4 !== null) {
    // Bitwise ops in JS are signed int32; `>>> 0` keeps comparisons unsigned
    // so 192.168/16 and 172.16/12 (high bit set) match correctly.
    const u = (mask: number, expect: number) => ((ipv4 & mask) >>> 0) === expect;
    // 0.0.0.0/8, 10.0.0.0/8, 169.254.0.0/16, 172.16.0.0/12, 192.168.0.0/16
    if (u(0xff000000, 0x00000000)) return true;
    if (u(0xff000000, 0x0a000000)) return true;
    if (u(0xffff0000, 0xa9fe0000)) return true;
    if (u(0xfff00000, 0xac100000)) return true;
    if (u(0xffff0000, 0xc0a80000)) return true;
  }
  if (host.includes(":")) {
    const h = host.toLowerCase();
    if (h === "::") return true;
    // Unique local (fc00::/7) and link-local (fe80::/10).
    if (
      h.startsWith("fc") ||
      h.startsWith("fd") ||
      h.startsWith("fe8") ||
      h.startsWith("fe9") ||
      h.startsWith("fea") ||
      h.startsWith("feb")
    ) {
      return true;
    }
  }
  return false;
}

/** Hostname of APP_URL, if set and parseable — the one private origin we allow. */
function appUrlHost(): string | null {
  const raw = process.env.APP_URL?.trim();
  if (!raw) return null;
  try {
    return new URL(raw).hostname.replace(/^\[|\]$/g, "").toLowerCase();
  } catch {
    return null;
  }
}

export type PublicUrlCheck =
  | { ok: true; url: URL }
  | { ok: false; reason: string };

/**
 * Parse `raw` and report whether a worker may navigate to it.
 */
export function checkPublicHttpUrl(raw: string): PublicUrlCheck {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return { ok: false, reason: "not a valid URL" };
  }

  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return { ok: false, reason: `scheme "${url.protocol}" is not allowed` };
  }

  const host = url.hostname.replace(/^\[|\]$/g, "").toLowerCase();
  if (!host) return { ok: false, reason: "URL has no hostname" };

  // Cloud instance metadata — never reachable, even via APP_URL.
  if (METADATA_HOSTNAMES.has(host) || host === "169.254.169.254") {
    return { ok: false, reason: `host "${host}" is not reachable from Ghost` };
  }

  if (isLoopbackHost(host)) {
    return { ok: true, url };
  }

  if (isPrivateNonLoopback(host)) {
    const allowed = appUrlHost();
    if (allowed && host === allowed) {
      return { ok: true, url };
    }
    return {
      ok: false,
      reason: `host "${host}" is a private or link-local address`,
    };
  }

  return { ok: true, url };
}

/** Throws with a short message when `raw` is not an allowed http(s) URL. */
export function assertPublicHttpUrl(raw: string): URL {
  const result = checkPublicHttpUrl(raw);
  if (!result.ok) throw new Error(`refusing to navigate: ${result.reason}`);
  return result.url;
}
