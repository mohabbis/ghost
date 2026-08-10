import { auth } from "@/auth";
import { createAgentToken } from "@ghost/core/agent-credentials";
import { isOrgAdmin } from "@ghost/core/roles";
import { prisma } from "@/lib/db";

export async function GET() {
  const session = await auth();
  if (!session?.user.id || !session.user.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const credentials = await prisma.agentCredential.findMany({
    where: {
      orgId: session.user.orgId,
      userId: session.user.id,
      revokedAt: null,
    },
    select: {
      id: true,
      name: true,
      tokenHint: true,
      createdAt: true,
      lastUsedAt: true,
      expiresAt: true,
    },
    orderBy: { createdAt: "desc" },
  });
  return Response.json({ credentials });
}

/** Generous enough for a real "annual rotation" policy, tight enough to catch a typo like 36500. */
const MAX_EXPIRES_IN_DAYS = 3650;

export async function POST(req: Request) {
  const session = await auth();
  if (!session?.user.id || !session.user.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }

  // Minting a credential that can start runs is an org-admin action. A MEMBER
  // can still *use* a key an admin handed them (or revoke their own), but they
  // cannot expand the attack surface by creating more.
  const membership = await prisma.membership.findUnique({
    where: {
      userId_orgId: { userId: session.user.id, orgId: session.user.orgId },
    },
    select: { role: true },
  });
  if (!membership || !isOrgAdmin(membership.role)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const body = (await req.json().catch(() => null)) as {
    name?: unknown;
    expiresInDays?: unknown;
  } | null;
  const name = typeof body?.name === "string" ? body.name.trim() : "";
  if (!name || name.length > 80) {
    return Response.json(
      { error: "name must be between 1 and 80 characters" },
      { status: 400 },
    );
  }

  let expiresAt: Date | null = null;
  if (body?.expiresInDays !== undefined && body.expiresInDays !== null) {
    const days = Number(body.expiresInDays);
    if (!Number.isInteger(days) || days < 1 || days > MAX_EXPIRES_IN_DAYS) {
      return Response.json(
        { error: `expiresInDays must be an integer between 1 and ${MAX_EXPIRES_IN_DAYS}` },
        { status: 400 },
      );
    }
    expiresAt = new Date(Date.now() + days * 24 * 60 * 60 * 1000);
  }

  const generated = createAgentToken();
  const credential = await prisma.agentCredential.create({
    data: {
      orgId: session.user.orgId,
      userId: session.user.id,
      name,
      tokenHash: generated.tokenHash,
      tokenHint: generated.tokenHint,
      expiresAt,
    },
    select: { id: true, name: true, tokenHint: true, createdAt: true, expiresAt: true },
  });
  return Response.json({ credential, token: generated.token }, { status: 201 });
}
