import type { NextRequest } from "next/server";
import { handlers } from "@/auth";
import { clientKey, rateLimit, tooManyRequests } from "@/lib/rate-limit";

/**
 * Auth.js route handlers, wrapped with a fixed-window rate limit.
 *
 * MFA verify / invite accept / audit verify are already throttled. Sign-in was
 * the remaining open door from the audit: an unthrottled `/api/auth/*` lets an
 * attacker grind credentials, magic-link tokens, and OAuth callbacks without
 * bound.
 *
 * POST (credentials / magic-link / callback) is the attack surface and is
 * keyed tightly. GET (session / csrf / providers) is hit by ordinary
 * `useSession` traffic, so it gets a looser budget that still bounds a
 * scripted sweep of token endpoints.
 *
 * Fail-closed via `rateLimit` itself: a Redis outage refuses the request
 * rather than letting auth through unmetered.
 */

async function limited(
  req: NextRequest,
  handle: (req: NextRequest) => Promise<Response>,
  { limit, windowSeconds, key }: { limit: number; windowSeconds: number; key: string },
): Promise<Response> {
  const result = await rateLimit(`${key}:${clientKey(req)}`, { limit, windowSeconds });
  if (!result.ok) return tooManyRequests(result);
  return handle(req);
}

export async function GET(req: NextRequest): Promise<Response> {
  return limited(req, handlers.GET, { limit: 120, windowSeconds: 60, key: "auth:get" });
}

export async function POST(req: NextRequest): Promise<Response> {
  return limited(req, handlers.POST, { limit: 20, windowSeconds: 60, key: "auth:post" });
}
