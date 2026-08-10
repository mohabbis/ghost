import { NextResponse } from "next/server";
import { agentToolsCatalog } from "@ghost/core/agent";
import { resolveAgentPrincipal } from "@/lib/agent-auth";

/**
 * Agent surface catalog.
 * GET /api/agent — tool list + human-approval contract (Ghost credential).
 */
export async function GET(req: Request) {
  const authz = await resolveAgentPrincipal(req);
  if (!authz.ok) {
    return NextResponse.json({ error: authz.error }, { status: authz.status });
  }

  return NextResponse.json({
    ...agentToolsCatalog(),
    principal: { via: authz.principal.via, orgId: authz.principal.orgId },
  });
}
