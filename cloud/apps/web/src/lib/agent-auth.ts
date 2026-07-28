import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { hashAgentToken } from "@ghost/core/agent-credentials";

/**
 * Principal for agent HTTP / MCP calls.
 *
 * Two paths:
 * 1. Browser session (NextAuth) — same as the dashboard.
 * 2. A revocable bearer credential created inside the authenticated Ghost app.
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
  return {
    ok: true,
    principal: {
      orgId: session.user.orgId,
      userId: session.user.id,
      via: "session",
    },
  };
}
