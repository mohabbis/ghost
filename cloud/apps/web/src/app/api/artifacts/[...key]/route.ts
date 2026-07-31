import { NextResponse } from "next/server";
import { readFile } from "node:fs/promises";
import { join, resolve, sep } from "node:path";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";

/**
 * Serve a run screenshot from the disk artifact store (dev). In production (S3)
 * this route is replaced by presigned URLs.
 *
 * The key shape is an **allow-list**, not a deny-list. The run artifact prefix
 * also holds `runs/<id>/session/gate-<n>.bin` — the encrypted browser session
 * captured at an approval gate, which contains live session cookies for the
 * customer's systems. A `..`-rejecting deny-list would happily serve that to
 * any authenticated member of the run's org. Only the two screenshot shapes
 * below are readable, and everything else 404s.
 */
const ALLOWED_FILENAME = /^(?:step|restore)-\d+\.png$/;

export async function GET(_req: Request, { params }: { params: Promise<{ key: string[] }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const { key } = await params;

  // Exactly ["runs", "<runId>", "step-<i>.png" | "restore-<i>.png"].
  if (
    key.length !== 3 ||
    key[0] !== "runs" ||
    !key[1] ||
    !ALLOWED_FILENAME.test(key[2] ?? "") ||
    key.some((seg) => seg.includes("\0") || seg.includes("/") || seg.includes("\\"))
  ) {
    return NextResponse.json({ error: "not found" }, { status: 404 });
  }

  const runId = key[1];
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
