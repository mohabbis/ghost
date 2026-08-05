import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import type { Job } from "bullmq";
import { prisma, Prisma } from "@ghost/core/db";
import type { PurgeArtifactsJob } from "@ghost/core/queue";

/**
 * DB-backed test for the artifact retention job.
 *
 * Mocks the artifact store rather than writing real files: the property under
 * test is *which runs* the job decides to touch and how it records that
 * decision (Run.artifactsPurgedAt, an audit event) — not the store's own
 * put/get/delete behavior, which packages/core/src/storage/artifacts.ts
 * already covers.
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

const store = vi.hoisted(() => ({
  deletePrefix: vi.fn(async (_prefix: string) => undefined),
}));
vi.mock("../storage/artifacts.js", () => ({ artifactStore: () => store }));

const { purgeArtifactsJob } = await import("./purgeArtifacts.js");

function fakeJob(): Job<PurgeArtifactsJob> {
  return { data: {}, id: "purge-test" } as Job<PurgeArtifactsJob>;
}

describe.skipIf(!hasDb)("purgeArtifactsJob (Postgres)", () => {
  let orgId: string;
  let workflowVersionId: string;
  const slug = `purge-${Date.now()}`;

  beforeAll(async () => {
    process.env.ARTIFACT_RETENTION_DAYS = "1";

    const org = await prisma.organization.create({ data: { name: "Purge Org", slug } });
    orgId = org.id;
    const wf = await prisma.workflow.create({
      data: {
        orgId,
        name: "purge demo",
        versions: {
          create: { version: 1, steps: [] as unknown as Prisma.InputJsonValue },
        },
      },
      include: { versions: true },
    });
    const [version] = wf.versions;
    if (!version) throw new Error("workflow version was not created");
    workflowVersionId = version.id;
  });

  afterEach(() => {
    store.deletePrefix.mockClear();
  });

  afterAll(async () => {
    delete process.env.ARTIFACT_RETENTION_DAYS;
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.$disconnect();
  });

  function daysAgo(n: number): Date {
    return new Date(Date.now() - n * 24 * 60 * 60 * 1000);
  }

  async function seedRun(data: {
    status: "SUCCEEDED" | "RUNNING" | "AWAITING_APPROVAL";
    endedAt: Date | null;
    artifactsPurgedAt?: Date | null;
  }) {
    return prisma.run.create({
      data: { orgId, workflowVersionId, ...data },
    });
  }

  it("purges a run that ended well before the retention window, and audits it", async () => {
    const run = await seedRun({ status: "SUCCEEDED", endedAt: daysAgo(3) });

    const result = await purgeArtifactsJob(fakeJob());

    // Not an exact-count assertion: this job intentionally scans across every
    // org, so it may also process eligible runs left by whichever other DB-backed
    // test files vitest happens to run concurrently against the same database.
    // The specific, deterministic claim is about this run and this org.
    expect(result.purged).toBeGreaterThanOrEqual(1);
    expect(store.deletePrefix).toHaveBeenCalledWith(`runs/${run.id}`);

    const updated = await prisma.run.findUniqueOrThrow({ where: { id: run.id } });
    expect(updated.artifactsPurgedAt).not.toBeNull();

    const audit = await prisma.auditEvent.findFirst({
      where: { orgId, entityType: "Run", entityId: run.id, action: "run.artifacts_purged" },
    });
    expect(audit).not.toBeNull();
    expect((audit?.metadata as { retentionDays?: number })?.retentionDays).toBe(1);
  });

  it("leaves a run that ended inside the retention window untouched", async () => {
    const run = await seedRun({ status: "SUCCEEDED", endedAt: new Date() });

    await purgeArtifactsJob(fakeJob());

    expect(store.deletePrefix).not.toHaveBeenCalledWith(`runs/${run.id}`);
    const unchanged = await prisma.run.findUniqueOrThrow({ where: { id: run.id } });
    expect(unchanged.artifactsPurgedAt).toBeNull();
  });

  it("never touches a run that has not ended, no matter how old", async () => {
    const run = await seedRun({ status: "RUNNING", endedAt: null });
    // Backdate createdAt directly — endedAt is what the job actually reads.
    await prisma.run.update({ where: { id: run.id }, data: { createdAt: daysAgo(365) } });

    await purgeArtifactsJob(fakeJob());

    expect(store.deletePrefix).not.toHaveBeenCalledWith(`runs/${run.id}`);
    const unchanged = await prisma.run.findUniqueOrThrow({ where: { id: run.id } });
    expect(unchanged.artifactsPurgedAt).toBeNull();
  });

  it("does not re-purge a run that was already purged", async () => {
    const run = await seedRun({
      status: "SUCCEEDED",
      endedAt: daysAgo(30),
      artifactsPurgedAt: daysAgo(1),
    });

    await purgeArtifactsJob(fakeJob());

    expect(store.deletePrefix).not.toHaveBeenCalledWith(`runs/${run.id}`);
  });

  it("leaves artifactsPurgedAt null and reports a failure when the store errors", async () => {
    const run = await seedRun({ status: "SUCCEEDED", endedAt: daysAgo(3) });
    store.deletePrefix.mockImplementationOnce(async () => {
      throw new Error("simulated store outage");
    });

    const result = await purgeArtifactsJob(fakeJob());

    expect(result.failed).toBeGreaterThanOrEqual(1);
    const unchanged = await prisma.run.findUniqueOrThrow({ where: { id: run.id } });
    expect(unchanged.artifactsPurgedAt).toBeNull();
  });
});
