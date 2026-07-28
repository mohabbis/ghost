import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { enqueueNoop } from "@/lib/queue";

/**
 * Phase 0 wiring smoke test: enqueue a no-op job the worker consumes. Proves the
 * web ↔ Redis ↔ worker path. Requires a session.
 */
export async function POST() {
  const session = await auth();
  if (!session?.user) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const job = await enqueueNoop({
    message: `hello from ${session.user.email ?? "web"}`,
    requestedAt: new Date().toISOString(),
  });

  return NextResponse.json({ enqueued: true, jobId: job.id });
}
