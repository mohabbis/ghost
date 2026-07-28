import { NextResponse } from "next/server";
import { resolveAgentPrincipal } from "@/lib/agent-auth";
import { invokeAgentTool } from "@/lib/agent-invoke";

/** GET /api/agent/runs/[id] — run status for agents. */
export async function GET(req: Request, { params }: { params: Promise<{ id: string }> }) {
  const authz = await resolveAgentPrincipal(req);
  if (!authz.ok) {
    return NextResponse.json({ error: authz.error }, { status: authz.status });
  }
  const { id } = await params;
  const out = await invokeAgentTool(authz.principal, "get_run", { runId: id });
  if (!out.ok) {
    return NextResponse.json({ error: out.error }, { status: out.status });
  }
  return NextResponse.json(out.result);
}
