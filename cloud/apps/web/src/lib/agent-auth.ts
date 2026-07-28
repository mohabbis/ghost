import { timingSafeEqual } from "node:crypto";
import { auth } from "@/auth";

/**
 * Principal for agent HTTP / MCP calls.
 *
 * Two paths:
 * 1. Browser session (NextAuth) — same as the dashboard.
 * 2. Bearer `GHOST_AGENT_API_KEY` — local MCP / early agent access, bound to
 *    `GHOST_AGENT_ORG_ID` + `GHOST_AGENT_USER_ID` (env). Per-org DB keys are
 *    follow-up; this is the honest early surface.
 */
export type AgentPrincipal = {
  userId: string;
  orgId: string;
  via: "session" | "api_key";
};

function safeEqualString(a: string, b: string): boolean {
  const ab = Buffer.from(a);
  const bb = Buffer.from(b);
  if (ab.length !== bb.length) return false;
  return timingSafeEqual(ab, bb);
}

function bearerToken(req: Request): string | null {
  const header = req.headers.get("authorization");
  if (!header) return null;
  const m = /^Bearer\s+(.+)$/i.exec(header.trim());
  return m?.[1]?.trim() || null;
}

/**
 * Resolve the calling agent. Returns a principal or an error payload.
 */
export async function resolveAgentPrincipal(
  req: Request,
): Promise<{ ok: true; principal: AgentPrincipal } | { ok: false; status: number; error: string }> {
  const token = bearerToken(req);
  const expected = process.env.GHOST_AGENT_API_KEY?.trim() || "";

  if (token) {
    if (!expected || !safeEqualString(token, expected)) {
      return { ok: false, status: 401, error: "invalid agent api key" };
    }
    const orgId = process.env.GHOST_AGENT_ORG_ID?.trim() || "";
    const userId = process.env.GHOST_AGENT_USER_ID?.trim() || "";
    if (!orgId || !userId) {
      return {
        ok: false,
        status: 503,
        error:
          "agent api key configured but GHOST_AGENT_ORG_ID / GHOST_AGENT_USER_ID are missing — set them to your org and user ids from the dashboard",
      };
    }
    return { ok: true, principal: { orgId, userId, via: "api_key" } };
  }

  const session = await auth();
  if (!session?.user?.orgId || !session.user.id) {
    return { ok: false, status: 401, error: "unauthorized" };
  }
  return {
    ok: true,
    principal: { orgId: session.user.orgId, userId: session.user.id, via: "session" },
  };
}
