import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer, type Server } from "node:http";
import { readFileSync } from "node:fs";
import type { Job } from "bullmq";
import { prisma, Prisma } from "@ghost/core/db";
import type { RunWorkflowJob } from "@ghost/core/queue";
import type { WorkflowSteps } from "@ghost/core/schema/step";
import { runWorkflowJob } from "./runWorkflow.js";

/**
 * DB-backed integration test for the approval halt/resume loop.
 *
 * Requires DATABASE_URL (docker-compose Postgres or any migrated ghost DB).
 * Serves the hermetic form fixture over HTTP so navigate URLs work like prod.
 * Skips cleanly when DATABASE_URL is unset so unit-only CI stays green.
 */

const hasDb = Boolean(process.env.DATABASE_URL);
const here = dirname(fileURLToPath(import.meta.url));
const fixtureHtml = readFileSync(
  join(here, "../browser/__fixtures__/form.html"),
  "utf8",
);

function fakeJob(runId: string, orgId: string): Job<RunWorkflowJob> {
  return { data: { runId, orgId } } as Job<RunWorkflowJob>;
}

describe.skipIf(!hasDb)("runWorkflowJob (Postgres)", () => {
  let server: Server;
  let fixtureBase: string;
  let artifactDir: string;
  let orgId: string;
  let userId: string;

  beforeAll(async () => {
    artifactDir = mkdtempSync(join(tmpdir(), "ghost-artifacts-"));
    process.env.GHOST_ARTIFACT_DIR = artifactDir;

    server = createServer((_req, res) => {
      res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      res.end(fixtureHtml);
    });
    await new Promise<void>((resolve) => {
      server.listen(0, "127.0.0.1", () => resolve());
    });
    const addr = server.address();
    if (!addr || typeof addr === "string") throw new Error("no listen address");
    fixtureBase = `http://127.0.0.1:${addr.port}`;

    const slug = `it-${Date.now()}`;
    const org = await prisma.organization.create({
      data: { name: "Integration Test Org", slug },
    });
    orgId = org.id;
    const user = await prisma.user.create({
      data: {
        email: `${slug}@example.com`,
        name: "IT",
        memberships: { create: { orgId, role: "OWNER" } },
      },
    });
    userId = user.id;
  });

  afterAll(async () => {
    await new Promise<void>((resolve, reject) => {
      server.close((err) => (err ? reject(err) : resolve()));
    });
    // Cascade cleans org-owned rows.
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.user.delete({ where: { id: userId } }).catch(() => undefined);
    rmSync(artifactDir, { recursive: true, force: true });
    await prisma.$disconnect();
  });

  async function seedRun(steps: WorkflowSteps) {
    const workflow = await prisma.workflow.create({
      data: {
        orgId,
        name: "IT demo",
        createdById: userId,
        versions: {
          create: {
            version: 1,
            steps: steps as unknown as Prisma.InputJsonValue,
          },
        },
      },
      include: { versions: true },
    });
    const version = workflow.versions[0]!;
    const run = await prisma.run.create({
      data: {
        orgId,
        workflowVersionId: version.id,
        triggeredById: userId,
        status: "QUEUED",
      },
    });
    return run.id;
  }

  const demoSteps = (): WorkflowSteps => [
    { id: "nav", type: "navigate", url: `${fixtureBase}/`, label: "Open order form" },
    {
      id: "fill",
      type: "fill",
      selector: { role: "textbox", name: "Full name" },
      value: "Ada Lovelace",
      sensitive: false,
      label: "Enter customer name",
    },
    {
      id: "submit",
      type: "click",
      selector: { role: "button", name: "Submit order" },
      label: "Submit order",
    },
    {
      id: "verify",
      type: "verify",
      assertion: { kind: "textPresent", expected: "Order submitted" },
      label: "Confirm submission",
    },
  ];

  it("halts at the sensitive click, then succeeds after approval (with prefix restore)", async () => {
    const runId = await seedRun(demoSteps());

    await runWorkflowJob(fakeJob(runId, orgId));

    let run = await prisma.run.findUniqueOrThrow({
      where: { id: runId },
      include: { steps: true, approvals: true },
    });
    expect(run.status).toBe("AWAITING_APPROVAL");
    expect(run.cursor).toBe(2);
    expect(run.approvals).toHaveLength(1);
    expect(run.approvals[0]!.status).toBe("PENDING");
    expect(run.steps.filter((s) => s.status === "SUCCEEDED")).toHaveLength(2);

    await prisma.approval.update({
      where: { id: run.approvals[0]!.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "QUEUED" } });

    await runWorkflowJob(fakeJob(runId, orgId));

    run = await prisma.run.findUniqueOrThrow({
      where: { id: runId },
      include: { steps: { orderBy: { index: "asc" } }, approvals: true },
    });
    expect(run.status).toBe("SUCCEEDED");
    expect(run.steps).toHaveLength(4);
    expect(run.steps.every((s) => s.status === "SUCCEEDED")).toBe(true);
    expect(run.steps[3]!.verification).toMatchObject({ passed: true });
  });

  it("fails the run when the pending approval is rejected", async () => {
    const runId = await seedRun(demoSteps());

    await runWorkflowJob(fakeJob(runId, orgId));

    const pending = await prisma.approval.findFirstOrThrow({
      where: { runId, status: "PENDING" },
    });
    await prisma.approval.update({
      where: { id: pending.id },
      data: { status: "REJECTED", resolvedById: userId, resolvedAt: new Date() },
    });
    // Mirror the web approve route's reject path (no re-enqueue).
    await prisma.run.update({
      where: { id: runId },
      data: { status: "FAILED", endedAt: new Date(), error: `rejected at step ${pending.stepIndex}` },
    });

    // A stray re-enqueue must be a no-op once FAILED.
    await runWorkflowJob(fakeJob(runId, orgId));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("FAILED");
    expect(run.error).toMatch(/rejected/);
  });
});
