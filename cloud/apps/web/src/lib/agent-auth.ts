import { cookies } from "next/headers";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { MFA_COOKIE_NAME, verifyMfaCookie } from "@/lib/mfa-cookie";
import { hashAgentToken } from "@ghost/core/agent-credentials";

/**
 * Principal for agent HTTP / MCP calls.
 *
 * Two paths:
 * 1. Browser session (NextAuth) — same as the dashboard.
 * 2. A revocable bearer credential created inside the authenticated Ghost app.
 *
 * **The second factor is enforced here, not in middleware.** `/api/agent` is
 * excluded from the middleware matcher because a bearer-credential caller has
 * no session at all and a session gate would reject every legitimate MCP
 * client. But the session path below is real: a browser cookie authenticates
 * these routes too, and `POST /api/agent/runs` starts a run against the
 * customer's systems. Without the check below, a stolen session cookie for an
 * MFA-enabled user could execute a workflow through `/api/agent` while the
 * identical action through `/api/runs` was correctly challenged — the second
 * factor covering everything except the endpoint that moves money.
 *
 * Bearer credentials are deliberately not MFA-gated: they are non-interactive,
 * already revocable and expirable, and there is nobody present to answer a
 * challenge. Their security story is issuance and revocation, not a second
 * factor.
 */
export type AgentPrincipal = {
  userId: string;
  orgId: string;
  via: "session" | "api_key";
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

  if (token) {
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

  const session = await auth();
  if (!session?.user?.orgId || !session.user.id) {
    return { ok: false, status: 401, error: "unauthorized" };
  }

  // Same verdict the middleware would reach for any other session-authenticated
  // route, reproduced here because the middleware cannot run on this path.
  if (session.user.mfaEnabled) {
    const jar = await cookies();
    const verified = await verifyMfaCookie(
      jar.get(MFA_COOKIE_NAME)?.value,
      session.user.id,
      session.user.sid,
    );
    if (!verified) {
      return { ok: false, status: 403, error: "two-factor verification required" };
    }
  }

  return {
    ok: true,
    principal: {
      orgId: session.user.orgId,
      userId: session.user.id,
      via: "session",
    },
  };
}
