import { NextResponse } from "next/server";
import { readFile } from "node:fs/promises";
import { join, resolve, sep } from "node:path";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { servableRunId } from "@ghost/core/artifacts";

/**
 * Serve a run screenshot from the disk artifact store (dev). In production (S3)
 * this route is replaced by presigned URLs.
 *
 * What may be served is an **allow-list**, not a deny-list, and it lives in
 * `@ghost/core/artifacts` alongside the key builders so the writer and this
 * gatekeeper cannot drift. The same run prefix holds
 * `runs/<id>/session/gate-<n>.bin` — the encrypted browser session captured at
 * an approval gate, containing live session cookies for the customer's systems.
 * A deny-list that merely rejected `..` would hand that to any authenticated
 * member of the run's org.
 */
export async function GET(_req: Request, { params }: { params: Promise<{ key: string[] }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const { key } = await params;
  const runId = servableRunId(key);
  if (!runId) return NextResponse.json({ error: "not found" }, { status: 404 });

  const owned = await prisma.run.findFirst({
    where: { id: runId, orgId: session.user.orgId },
    select: { id: true },
  });
  if (!owned) return NextResponse.json({ error: "not found" }, { status: 404 });

  const baseDir = resolve(process.env.GHOST_ARTIFACT_DIR ?? resolve(process.cwd(), ".artifacts"));
  const full = resolve(join(baseDir, ...key));
  // Belt and braces: the shape check above already forbids traversal, but the
  // resolved path must still sit inside the artifact root.
  if (full !== baseDir && !full.startsWith(baseDir + sep)) {
    return NextResponse.json({ error: "not found" }, { status: 404 });
  }

  try {
    const bytes = await readFile(full);
    return new Response(new Uint8Array(bytes), {
      headers: { "content-type": "image/png", "cache-control": "private, max-age=60" },
    });
  } catch {
    return NextResponse.json({ error: "not found" }, { status: 404 });
  }
}
