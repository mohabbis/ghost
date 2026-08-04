import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { buildRunView } from "@/lib/run-view";

/** Run detail for polling: run status + ordered steps + pending approvals. */
export async function GET(_req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const { id } = await params;

  const view = await buildRunView(session.user.orgId, session.user.id, id);
  if (!view) return NextResponse.json({ error: "not found" }, { status: 404 });

  return NextResponse.json(view);
}
