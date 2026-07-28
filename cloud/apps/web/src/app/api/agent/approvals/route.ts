import { NextResponse } from "next/server";
import { resolveAgentPrincipal } from "@/lib/agent-auth";
import { invokeAgentTool } from "@/lib/agent-invoke";

/**
 * GET /api/agent/approvals — pending human approvals (read-only).
 * There is intentionally no POST here; agents cannot approve.
 */
export async function GET(req: Request) {
  const authz = await resolveAgentPrincipal(req);
  if (!authz.ok) {
    return NextResponse.json({ error: authz.error }, { status: authz.status });
  }

  const out = await invokeAgentTool(authz.principal, "list_pending_approvals", {});
  if (!out.ok) {
    return NextResponse.json({ error: out.error }, { status: out.status });
  }
  return NextResponse.json(out.result);
}

export async function POST() {
  return NextResponse.json(
    {
      error:
        "Agents cannot approve or reject. Open the run in the Ghost UI; a human must decide.",
    },
    { status: 403 },
  );
}
