/**
 * Fail-closed checks on the authentication environment.
 *
 * Two things here are deliberately paranoid, because both failure modes are
 * silent — the app boots, sign-in works, and nothing looks wrong:
 *
 *   1. `AUTH_SECRET` signs the session JWT. `.env.example` ships a working
 *      dev value on purpose (so `cp .env.example .env` yields a running
 *      stack, the same rationale as `GHOST_SESSION_KEY`), which means the
 *      exact string that signs your sessions may be published in this repo.
 *      Anyone holding it can mint a token for any user in any organization.
 *      In production that is not a warning, it is a refusal to start.
 *
 *   2. The dev sign-in provider accepts *any* email with no credential at
 *      all. It was gated on `NODE_ENV !== "production"` alone — one
 *      environment variable, frequently unset, absent from many container
 *      images, and not something anyone treats as a security boundary. A
 *      deployment that forgets it hands out "sign in as anyone".
 */

/** The value `.env.example` ships. Present in the repo, therefore public. */
export const PUBLISHED_DEV_AUTH_SECRET = "dev-only-change-me-openssl-rand-base64-32";

const MIN_SECRET_LENGTH = 32;

export function isProduction(): boolean {
  return process.env.NODE_ENV === "production";
}

/**
 * True while `next build` is compiling, as opposed to serving traffic.
 *
 * The build runs with `NODE_ENV=production` and imports every route module to
 * collect page data, so an unguarded production check fails the build on any
 * machine without production secrets — including CI, which has no business
 * holding them. Building is not serving: the secret is required to issue a
 * session, and the assertion still runs when the module is first imported to
 * handle a real request.
 */
export function isBuildPhase(): boolean {
  return process.env.NEXT_PHASE === "phase-production-build";
}

function authUrl(): string {
  return process.env.AUTH_URL ?? process.env.NEXTAUTH_URL ?? "";
}

/**
 * True when this instance is reachable only from the machine running it.
 *
 * Used as a second, independent signal to `NODE_ENV`: a Ghost anyone else can
 * reach is not a local dev box, whatever `NODE_ENV` claims. An unparseable or
 * absent URL is treated as *not* loopback — unknown means "assume exposed".
 */
export function isLoopbackInstance(): boolean {
  const raw = authUrl();
  if (!raw) return false;
  let host: string;
  try {
    host = new URL(raw).hostname;
  } catch {
    return false;
  }
  return host === "localhost" || host === "127.0.0.1" || host === "::1" || host === "[::1]";
}

/**
 * Whether the passwordless "any email" dev provider may be registered.
 *
 * Requires production to be ruled out **and** the instance to be loopback-only
 * **and** no explicit opt-out — so forgetting any single one of them fails
 * safe rather than exposing the provider.
 */
export function devSignInAllowed(): boolean {
  if (isProduction()) return false;
  if (process.env.GHOST_DISABLE_DEV_SIGNIN === "true") return false;
  return isLoopbackInstance();
}

export class AuthEnvironmentError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AuthEnvironmentError";
  }
}

/**
 * Refuse to run with a session secret that cannot protect a session.
 *
 * Throws at import time so a misconfigured deployment dies at boot with a
 * readable reason, rather than serving traffic with forgeable sessions.
 * Outside production this only warns: breaking `pnpm dev` for a dev secret
 * would just push people back to the published one.
 */
export function assertAuthSecretUsable(): void {
  const secret = process.env.AUTH_SECRET;

  // Compiling, not serving — see `isBuildPhase`.
  if (isBuildPhase()) return;

  if (!isProduction()) {
    if (secret === PUBLISHED_DEV_AUTH_SECRET) {
      console.warn(
        "[auth] Using the AUTH_SECRET from .env.example. Fine locally; this value is public, so production will refuse to start with it.",
      );
    }
    return;
  }

  if (!secret) {
    throw new AuthEnvironmentError(
      "AUTH_SECRET is not set. Generate one with `openssl rand -base64 32`. Refusing to start: without it session tokens cannot be trusted.",
    );
  }
  if (secret === PUBLISHED_DEV_AUTH_SECRET) {
    throw new AuthEnvironmentError(
      "AUTH_SECRET is the published example value from .env.example. It is in the repository, so anyone can forge a session for any user in any organization. Generate a real one with `openssl rand -base64 32`.",
    );
  }
  if (secret.length < MIN_SECRET_LENGTH) {
    throw new AuthEnvironmentError(
      `AUTH_SECRET is ${secret.length} characters; at least ${MIN_SECRET_LENGTH} are required. Generate one with \`openssl rand -base64 32\`.`,
    );
  }
}

/**
 * How long a session token stays valid. Auth.js defaults to 30 days, which is
 * a long time for a product that can move money in the systems it operates.
 * Override with `GHOST_SESSION_MAX_AGE_SECONDS` where a different policy
 * applies.
 */
export function sessionMaxAgeSeconds(): number {
  const raw = Number(process.env.GHOST_SESSION_MAX_AGE_SECONDS);
  if (Number.isFinite(raw) && raw > 0) return Math.floor(raw);
  return 12 * 60 * 60;
}
