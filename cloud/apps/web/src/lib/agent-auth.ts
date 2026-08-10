import { prisma } from "@/lib/db";
import { hashAgentToken } from "@ghost/core/agent-credentials";

/**
 * Principal for agent HTTP / MCP calls.
 *
 * **Bearer credential only.** A browser session cookie is deliberately
 * refused here. `/api/agent` is excluded from the middleware matcher because a
 * bearer-credential caller has no session at all — and that same exclusion
 * used to mean a stolen session cookie could `POST /api/agent/runs` and start
 * a real run against the customer's systems while the identical action through
 * `/api/runs` was MFA-gated. Closing that hole by re-checking MFA on the
 * session path was a bandage; the honest boundary is that agents authenticate
 * with a revocable, expirable credential minted in Settings, and humans use
 * the dashboard routes.
 *
 * Bearer credentials are not MFA-gated: they are non-interactive, already
 * revocable and expirable, and there is nobody present to answer a challenge.
 * Their security story is issuance and revocation, not a second factor.
 */
export type AgentPrincipal = {
  userId: string;
  orgId: string;
  via: "api_key";
};

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
): Promise<
  | { ok: true; principal: AgentPrincipal }
  | { ok: false; status: number; error: string }
> {
  const token = bearerToken(req);

  if (!token) {
    // A session cookie is not enough. Even with MFA answered, the agent surface
    // is for non-interactive clients that hold a minted credential — not for
    // the browser. Dashboard actions go through `/api/runs` et al., which the
    // middleware MFA-gates. Distinguishing "has a session" from "has nothing"
    // is unnecessary: both get 401, and the message tells a browser caller how
    // to proceed.
    return {
      ok: false,
      status: 401,
      error: "agent api key required — create one in Settings",
    };
  }

  const credential = await prisma.agentCredential.findUnique({
    where: { tokenHash: hashAgentToken(token) },
    select: {
      id: true,
      orgId: true,
      userId: true,
      revokedAt: true,
      expiresAt: true,
    },
  });
  if (
    !credential ||
    credential.revokedAt ||
    (credential.expiresAt && credential.expiresAt <= new Date())
  ) {
    return { ok: false, status: 401, error: "invalid agent api key" };
  }
  const membership = await prisma.membership.findUnique({
    where: {
      userId_orgId: { userId: credential.userId, orgId: credential.orgId },
    },
    select: { id: true },
  });
  if (!membership)
    return { ok: false, status: 401, error: "invalid agent api key" };
  await prisma.agentCredential.update({
    where: { id: credential.id },
    data: { lastUsedAt: new Date() },
  });
  return {
    ok: true,
    principal: {
      orgId: credential.orgId,
      userId: credential.userId,
      via: "api_key",
    },
  };
}
