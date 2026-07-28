import { auth } from "@/auth";
import { createAgentToken } from "@ghost/core/agent-credentials";
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

export async function POST(req: Request) {
  const session = await auth();
  if (!session?.user.id || !session.user.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const body = (await req.json().catch(() => null)) as {
    name?: unknown;
  } | null;
  const name = typeof body?.name === "string" ? body.name.trim() : "";
  if (!name || name.length > 80) {
    return Response.json(
      { error: "name must be between 1 and 80 characters" },
      { status: 400 },
    );
  }
  const generated = createAgentToken();
  const credential = await prisma.agentCredential.create({
    data: {
      orgId: session.user.orgId,
      userId: session.user.id,
      name,
      tokenHash: generated.tokenHash,
      tokenHint: generated.tokenHint,
    },
    select: { id: true, name: true, tokenHint: true, createdAt: true },
  });
  return Response.json({ credential, token: generated.token }, { status: 201 });
}
