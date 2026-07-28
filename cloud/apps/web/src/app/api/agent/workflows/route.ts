import { NextResponse } from "next/server";
import { resolveAgentPrincipal } from "@/lib/agent-auth";
import { invokeAgentTool } from "@/lib/agent-invoke";

/** GET /api/agent/workflows — list org workflows for agents. */
export async function GET(req: Request) {
  const authz = await resolveAgentPrincipal(req);
  if (!authz.ok) {
    return NextResponse.json({ error: authz.error }, { status: authz.status });
  }

  const url = new URL(req.url);
  const includeArchived = url.searchParams.get("includeArchived") === "true";
  const out = await invokeAgentTool(authz.principal, "list_workflows", { includeArchived });
  if (!out.ok) {
    return NextResponse.json({ error: out.error }, { status: out.status });
  }
  return NextResponse.json(out.result);
}
