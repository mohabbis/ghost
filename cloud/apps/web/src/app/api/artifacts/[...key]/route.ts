import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { servableRunId } from "@ghost/core/artifacts";
import { artifactStore } from "@ghost/core/storage/artifacts";

/**
 * Serve a run screenshot, from disk in dev and from S3 in production.
 *
 * What may be served is an **allow-list**, not a deny-list, and it lives in
 * `@ghost/core/artifacts` alongside the key builders so the writer and this
 * gatekeeper cannot drift. The same run prefix holds
 * `runs/<id>/session/gate-<n>.bin` — the encrypted browser session captured at
 * an approval gate, containing live session cookies for the customer's systems.
 * A deny-list that merely rejected `..` would hand that to any authenticated
 * member of the run's org.
 *
 * Both backends come through this route rather than S3 objects being linked
 * directly. That is the point: these checks are the only thing between a caller
 * and a session blob, and a signed URL minted before them would be a hole no
 * later check could close. **Order is the security property** — authenticate,
 * allow-list the key shape, confirm the org owns the run, and only then ask the
 * store for a URL.
 */

/** Long enough to follow a redirect, short enough to be dull if it leaks. */
const SIGNED_URL_TTL_SECONDS = 60;

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

  // Every check has passed: the key shape is servable and the org owns the run.
  const store = artifactStore();
  const objectKey = key.join("/");

  // S3 hands the fetch to the browser; disk returns null and we serve it here.
  // A signing failure falls through to the read rather than 500ing, because a
  // misconfigured presigner should not take the approval UI down with it.
  const url = await store.signedUrl(objectKey, SIGNED_URL_TTL_SECONDS).catch(() => null);
  if (url) return NextResponse.redirect(url, 307);

  try {
    // The store's own containment check rejects any key escaping the root.
    const bytes = await store.get(objectKey);
    return new Response(new Uint8Array(bytes), {
      headers: { "content-type": "image/png", "cache-control": "private, max-age=60" },
    });
  } catch {
    return NextResponse.json({ error: "not found" }, { status: 404 });
  }
}
