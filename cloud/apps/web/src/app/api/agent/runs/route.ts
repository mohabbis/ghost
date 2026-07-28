import { NextResponse } from "next/server";
import { resolveAgentPrincipal } from "@/lib/agent-auth";
import { invokeAgentTool } from "@/lib/agent-invoke";

/** POST /api/agent/runs { workflowId } — enqueue a run (approval still human-only). */
export async function POST(req: Request) {
  const authz = await resolveAgentPrincipal(req);
  if (!authz.ok) {
    return NextResponse.json({ error: authz.error }, { status: authz.status });
  }

  const body = (await req.json().catch(() => ({}))) as { workflowId?: string };
  const out = await invokeAgentTool(authz.principal, "start_run", {
    workflowId: body.workflowId,
  });
  if (!out.ok) {
    return NextResponse.json({ error: out.error }, { status: out.status });
  }
  return NextResponse.json(out.result);
}
