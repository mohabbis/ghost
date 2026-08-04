import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { prisma } from "@ghost/core/db";
import { finalizeIfTerminal } from "@/lib/recording-compiler";

/**
 * `finalizeIfTerminal` is the one place an AI-produced file becomes
 * `Recording.compiledSteps` — this pins that a malformed or role-mismatched
 * proposal is rejected (FAILED, with a human-readable reason) rather than
 * ever landing unvalidated. It never reaches `WorkflowVersion.steps`; the
 * publish path re-validates the same way regardless of what's stored here.
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

const hr = vi.hoisted(() => ({
  session: { status: "done", turn_status: "done" } as { status: string; turn_status: string },
  files: [] as { path: string; file_id: string; bytes: number; media_type: string; download_url: string }[],
  fileContents: {} as Record<string, string>,
}));

vi.mock("@/lib/harness-router", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/harness-router")>();
  return {
    ...actual,
    getSession: async () => hr.session,
    getSessionFiles: async () => hr.files,
    downloadSessionFile: async (_sessionId: string, fileId: string) => ({
      buffer: Buffer.from(hr.fileContents[fileId] ?? ""),
      contentType: "application/octet-stream",
    }),
  };
});

function stepsFile(content: unknown) {
  hr.files = [
    { path: "steps.json", file_id: "f1", bytes: 0, media_type: "application/json", download_url: "" },
  ];
  hr.fileContents.f1 = JSON.stringify(content);
}

describe.skipIf(!hasDb)("finalizeIfTerminal (Postgres)", () => {
  let orgId: string;
  const slug = `rec-compile-${Date.now()}`;

  beforeAll(async () => {
    const org = await prisma.organization.create({ data: { name: "Compile", slug } });
    orgId = org.id;
  });

  afterAll(async () => {
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
  });

  beforeEach(() => {
    hr.session = { status: "done", turn_status: "done" };
    hr.files = [];
    hr.fileContents = {};
  });

  async function running() {
    return prisma.recording.create({
      data: { orgId, status: "STOPPED", compileStatus: "RUNNING", harnessSessionId: "sess-1" },
    });
  }

  it("accepts a valid steps.json and stores it as READY", async () => {
    const recording = await running();
    stepsFile([{ id: "n1", type: "navigate", url: "https://example.com" }]);

    const result = await finalizeIfTerminal(recording.id);
    expect(result).toEqual({ compileStatus: "READY", terminal: true });

    const row = await prisma.recording.findUniqueOrThrow({ where: { id: recording.id } });
    expect(row.compiledSteps).toEqual([{ id: "n1", type: "navigate", url: "https://example.com" }]);
    expect(row.compileError).toBeNull();
  });

  it("rejects a role_mismatch response instead of storing it as steps", async () => {
    const recording = await running();
    stepsFile({ role_mismatch: true, reason: "asked to build the host app" });

    const result = await finalizeIfTerminal(recording.id);
    expect(result.compileStatus).toBe("FAILED");

    const row = await prisma.recording.findUniqueOrThrow({ where: { id: recording.id } });
    expect(row.compiledSteps).toBeNull();
    expect(row.compileError).toContain("asked to build the host app");
  });

  it("rejects steps that fail schema validation", async () => {
    const recording = await running();
    stepsFile([{ id: "bad", type: "navigate" /* missing required url */ }]);

    const result = await finalizeIfTerminal(recording.id);
    expect(result.compileStatus).toBe("FAILED");

    const row = await prisma.recording.findUniqueOrThrow({ where: { id: recording.id } });
    expect(row.compiledSteps).toBeNull();
    expect(row.compileError).toContain("failed validation");
  });

  it("surfaces a non-success terminal session status without treating it as READY", async () => {
    const recording = await running();
    hr.session = { status: "incomplete", turn_status: "incomplete" };

    const result = await finalizeIfTerminal(recording.id);
    expect(result.compileStatus).toBe("FAILED");

    const row = await prisma.recording.findUniqueOrThrow({ where: { id: recording.id } });
    expect(row.compileError).toContain("incomplete");
  });

  it("is a no-op once the recording has already left RUNNING", async () => {
    const recording = await prisma.recording.create({
      data: { orgId, status: "STOPPED", compileStatus: "READY" },
    });

    const result = await finalizeIfTerminal(recording.id);
    expect(result).toEqual({ compileStatus: "READY", terminal: true });
  });
});
