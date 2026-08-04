import "server-only";
import IORedis from "ioredis";

/**
 * Fixed-window rate limiting for the endpoints an attacker gets to call
 * repeatedly without being authenticated as anyone in particular.
 *
 * The one that matters most is `POST /api/mfa/verify`. A TOTP code is six
 * digits and `verifyTotp` accepts a ±1-step skew window, so an unthrottled
 * endpoint reduces the second factor to roughly a million guesses — minutes of
 * scripted requests. A second factor you can brute-force is not a second
 * factor, and it is the control the product's approval story leans on.
 *
 * Redis-backed rather than in-memory because the web app runs as multiple
 * serverless instances: a per-process counter would give an attacker one full
 * budget per instance, and instances are created on demand.
 *
 * **Fails closed.** If Redis cannot be reached the request is refused rather
 * than allowed through unmetered. That is a deliberate choice and it is not
 * free — a Redis outage stops people answering an MFA challenge. It is
 * defensible here because Redis is already load-bearing: with it down, runs do
 * not execute at all, so this does not introduce a new failure mode, it
 * matches an existing one. The alternative — failing open — means the outage
 * window is exactly when brute force becomes free.
 */

const globalForRateLimit = globalThis as unknown as { ghostRateLimitRedis?: IORedis };

function connection(): IORedis {
  if (!globalForRateLimit.ghostRateLimitRedis) {
    const url = process.env.REDIS_URL;
    if (!url) throw new Error("REDIS_URL is not set — see cloud/.env.example");
    globalForRateLimit.ghostRateLimitRedis = new IORedis(url, { maxRetriesPerRequest: null });
  }
  return globalForRateLimit.ghostRateLimitRedis;
}

export interface RateLimitResult {
  ok: boolean;
  /** Seconds the caller should wait. 0 when `ok`. */
  retryAfter: number;
}

/**
 * Counts one hit against `key` and reports whether it exceeded `limit` within
 * `windowSeconds`.
 *
 * Fixed window, not a sliding one: an attacker can land `2 * limit` requests
 * across a window boundary. That is a known and accepted property — the goal
 * is to turn a million guesses into an infeasible number, not to be exact —
 * and it buys an implementation with one round trip and no stored history.
 */
export async function rateLimit(
  key: string,
  { limit, windowSeconds }: { limit: number; windowSeconds: number },
): Promise<RateLimitResult> {
  const redisKey = `ghost:rl:${key}`;
  try {
    const redis = connection();
    // INCR then EXPIRE-if-new. Pipelined so this is a single round trip; the
    // TTL is set only when the counter was just created, so the window starts
    // at the first hit rather than sliding forward with every later one.
    const [[, countRaw]] = (await redis
      .multi()
      .incr(redisKey)
      .expire(redisKey, windowSeconds, "NX")
      .exec()) as [[Error | null, number], [Error | null, number]];

    const count = Number(countRaw);
    if (count > limit) {
      const ttl = await redis.ttl(redisKey);
      return { ok: false, retryAfter: ttl > 0 ? ttl : windowSeconds };
    }
    return { ok: true, retryAfter: 0 };
  } catch {
    // See the fail-closed note above.
    return { ok: false, retryAfter: windowSeconds };
  }
}

/**
 * A best-effort client identifier for unauthenticated endpoints.
 *
 * Trusts `x-forwarded-for`'s first entry, which is correct behind Vercel and
 * any proxy that rewrites the header, and forgeable if the app is ever exposed
 * directly. Treat it as a way to make casual scripted abuse expensive, not as
 * an identity — the per-subject limits below are the ones that actually bound
 * an attack on a specific account.
 */
export function clientKey(req: Request): string {
  const forwarded = req.headers.get("x-forwarded-for");
  const ip = forwarded?.split(",")[0]?.trim();
  return ip || req.headers.get("x-real-ip") || "unknown";
}

/** 429 with a `Retry-After`, so a well-behaved client backs off correctly. */
export function tooManyRequests(result: RateLimitResult): Response {
  return Response.json(
    { error: "too many requests" },
    { status: 429, headers: { "Retry-After": String(result.retryAfter) } },
  );
}
