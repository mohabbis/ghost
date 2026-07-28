import { NextResponse } from "next/server";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";

/**
 * Serve a run screenshot from the disk artifact store (dev). Keys look like
 * `runs/<runId>/step-<i>.png`; access is org-scoped by verifying the run's org
 * before streaming. In production (S3) this route is replaced by presigned URLs.
 */
export async function GET(_req: Request, { params }: { params: Promise<{ key: string[] }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const { key } = await params;
  if (key.some((seg) => seg === ".." || seg.includes("\0"))) {
    return NextResponse.json({ error: "bad key" }, { status: 400 });
  }

  // key = ["runs", "<runId>", "step-<i>.png"] — enforce org ownership of the run.
  const runId = key[0] === "runs" ? key[1] : undefined;
  if (!runId) return NextResponse.json({ error: "bad key" }, { status: 400 });
  const owned = await prisma.run.findFirst({
    where: { id: runId, orgId: session.user.orgId },
    select: { id: true },
  });
  if (!owned) return NextResponse.json({ error: "not found" }, { status: 404 });

  const baseDir = process.env.GHOST_ARTIFACT_DIR ?? resolve(process.cwd(), ".artifacts");
  try {
    const bytes = await readFile(join(baseDir, ...key));
    return new Response(new Uint8Array(bytes), {
      headers: { "content-type": "image/png", "cache-control": "private, max-age=60" },
    });
  } catch {
    return NextResponse.json({ error: "not found" }, { status: 404 });
  }
}
