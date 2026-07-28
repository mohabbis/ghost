import { NextResponse } from "next/server";
import { resolveAgentPrincipal } from "@/lib/agent-auth";
import { invokeAgentTool } from "@/lib/agent-invoke";

/**
 * Invoke an allow-listed agent tool.
 * POST /api/agent/invoke  { "name": "list_workflows", "arguments": {} }
 *
 * Approval tools are rejected with 403 — humans approve in the Ghost UI only.
 */
export async function POST(req: Request) {
  const authz = await resolveAgentPrincipal(req);
  if (!authz.ok) {
    return NextResponse.json({ error: authz.error }, { status: authz.status });
  }

  const body = (await req.json().catch(() => null)) as {
    name?: unknown;
    arguments?: unknown;
    args?: unknown;
  } | null;

  const name = typeof body?.name === "string" ? body.name.trim() : "";
  if (!name) {
    return NextResponse.json({ error: "name required" }, { status: 400 });
  }

  const args = body?.arguments ?? body?.args ?? {};
  const out = await invokeAgentTool(authz.principal, name, args);
  if (!out.ok) {
    return NextResponse.json({ error: out.error }, { status: out.status });
  }
  return NextResponse.json({ name, result: out.result });
}
