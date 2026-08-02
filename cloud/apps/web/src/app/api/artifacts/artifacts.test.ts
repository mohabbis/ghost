import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { prisma } from "@ghost/core/db";
import { screenshotKey, sessionStateKey } from "@ghost/core/artifacts";

/**
 * The artifact route is the only thing standing between a caller and the
 * encrypted browser session captured at an approval gate — live cookies for the
 * customer's systems, sitting under the same `runs/<id>/` prefix as the
 * screenshots this route exists to serve.
 *
 * With S3 the route hands out a **presigned URL**, which is a bearer token: it
 * works without a session for as long as it lives. So the property under test
 * is not just "session blobs 404" but "the signer is never reached for one" —
 * a 404 returned *after* minting a URL would still have minted it.
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

// Typed parameters so the TTL assertion below can read the call arguments.
const store = vi.hoisted(() => ({
  signedUrl: vi.fn(
    async (_key: string, _ttlSeconds: number): Promise<string | null> =>
      "https://s3.example.com/signed?sig=abc",
  ),
  get: vi.fn(async (_key: string) => Buffer.from("png-bytes")),
}));
vi.mock("@ghost/core/storage/artifacts", () => ({ artifactStore: () => store }));

const session = vi.hoisted(() => ({
  current: null as null | { user: { id: string; orgId: string } },
}));
vi.mock("@/auth", () => ({ auth: async () => session.current }));

describe.skipIf(!hasDb)("GET /api/artifacts (Postgres)", () => {
  let orgA: string;
  let orgB: string;
  let runId: string;
  const slug = `artifacts-${Date.now()}`;

  beforeAll(async () => {
    const a = await prisma.organization.create({ data: { name: "A", slug: `${slug}-a` } });
    const b = await prisma.organization.create({ data: { name: "B", slug: `${slug}-b` } });
    orgA = a.id;
    orgB = b.id;
    const wf = await prisma.workflow.create({
      data: {
        orgId: orgA,
        name: "Artifacts",
        versions: { create: { version: 1, steps: [] as never } },
      },
      include: { versions: true },
    });
    const run = await prisma.run.create({
      data: { orgId: orgA, workflowVersionId: wf.versions[0]!.id, status: "SUCCEEDED" },
    });
    runId = run.id;
  });

  afterAll(async () => {
    await prisma.organization.deleteMany({ where: { id: { in: [orgA, orgB] } } });
  });

  beforeEach(() => {
    store.signedUrl.mockClear();
    store.get.mockClear();
    session.current = { user: { id: "u", orgId: orgA } };
  });

  async function fetchKey(key: string) {
    const { GET } = await import("./[...key]/route.js");
    return GET(new Request(`http://localhost/api/artifacts/${key}`), {
      params: Promise.resolve({ key: key.split("/") }),
    });
  }

  it("redirects an allow-listed screenshot to a signed URL", async () => {
    const res = await fetchKey(screenshotKey(runId, 2));

    expect(res.status).toBe(307);
    expect(res.headers.get("location")).toBe("https://s3.example.com/signed?sig=abc");
    expect(store.signedUrl).toHaveBeenCalledOnce();
    // Short-lived: a leaked URL should stop working almost immediately.
    expect(store.signedUrl.mock.calls[0]![1]).toBeLessThanOrEqual(300);
  });

  it("never signs a session blob, even for a member of the owning org", async () => {
    const res = await fetchKey(sessionStateKey(runId, 2));

    expect(res.status).toBe(404);
    // The assertion that matters. A 404 produced *after* signing would still
    // have handed out a working URL for the customer's session cookies.
    expect(store.signedUrl).not.toHaveBeenCalled();
    expect(store.get).not.toHaveBeenCalled();
  });

  it("never signs for another org's run", async () => {
    session.current = { user: { id: "u", orgId: orgB } };
    const res = await fetchKey(screenshotKey(runId, 2));

    expect(res.status).toBe(404);
    expect(store.signedUrl).not.toHaveBeenCalled();
  });

  it("never signs for an unauthenticated caller", async () => {
    session.current = null;
    const res = await fetchKey(screenshotKey(runId, 2));

    expect(res.status).toBe(401);
    expect(store.signedUrl).not.toHaveBeenCalled();
  });

  it("streams the bytes when the store cannot sign (disk store)", async () => {
    store.signedUrl.mockResolvedValueOnce(null as unknown as string);
    const res = await fetchKey(screenshotKey(runId, 2));

    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toBe("image/png");
    expect(Buffer.from(await res.arrayBuffer()).toString()).toBe("png-bytes");
  });

  it("falls back to streaming rather than 500ing when signing fails", async () => {
    // A misconfigured presigner should not take the approval UI down: the
    // screenshot is the evidence a human approves against.
    store.signedUrl.mockRejectedValueOnce(new Error("bad credentials"));
    const res = await fetchKey(screenshotKey(runId, 2));

    expect(res.status).toBe(200);
    expect(store.get).toHaveBeenCalledOnce();
  });
});
