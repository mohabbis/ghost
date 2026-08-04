import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { vi } from "vitest";
import { prisma } from "@ghost/core/db";
import type { WorkflowSteps } from "@ghost/core/schema/step";

/**
 * Run-detail SSE push.
 *
 * `/api/runs/[id]/stream` replaces the old `setInterval`-based client
 * polling of `GET /api/runs/[id]` every 1.5s. It's a plain Route Handler
 * returning a `ReadableStream`, so — unlike the workflow-editor UI work —
 * this is directly testable without a browser: read the stream to
 * completion and parse the `data: ...` frames it wrote.
 *
 * Two behaviors this pins:
 *   - a run already in a terminal state gets exactly one event and the
 *     stream ends promptly, rather than idling out the full segment;
 *   - a run that changes mid-stream (QUEUED -> RUNNING -> SUCCEEDED) is
 *     observed as multiple distinct events, not just the final one — the
 *     whole reason this exists is to push intermediate state, not only
 *     the end result.
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

const session = vi.hoisted(() => ({
  current: null as null | { user: { id: string; orgId: string } },
}));
vi.mock("@/auth", () => ({ auth: async () => session.current }));

const steps: WorkflowSteps = [{ id: "nav", type: "navigate", url: "https://example.com" }];

describe.skipIf(!hasDb)("run stream (Postgres)", () => {
  let orgId: string;
  let userId: string;
  const slug = `stream-${Date.now()}`;

  beforeAll(async () => {
    const org = await prisma.organization.create({ data: { name: "Stream", slug } });
    orgId = org.id;
    const user = await prisma.user.create({
      data: { email: `${slug}@example.com`, memberships: { create: { orgId, role: "OWNER" } } },
    });
    userId = user.id;
  });

  afterAll(async () => {
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.user.delete({ where: { id: userId } }).catch(() => undefined);
  });

  beforeEach(() => {
    session.current = { user: { id: userId, orgId } };
  });

  async function runIn(status: "QUEUED" | "SUCCEEDED") {
    const wf = await prisma.workflow.create({
      data: {
        orgId,
        name: `wf-${Math.random().toString(36).slice(2, 8)}`,
        versions: { create: { version: 1, steps: steps as never } },
      },
      include: { versions: true },
    });
    const run = await prisma.run.create({
      data: { orgId, workflowVersionId: wf.versions[0]!.id, triggeredById: userId, status },
    });
    return run.id;
  }

  /** Read an SSE response to completion and parse its `data: ...` frames. */
  async function readEvents(res: Response): Promise<unknown[]> {
    const text = await new Response(res.body).text();
    return text
      .split("\n\n")
      .filter((frame) => frame.startsWith("data: "))
      .map((frame) => JSON.parse(frame.slice("data: ".length)));
  }

  /**
   * Read an SSE response frame by frame, running `onEvent` after each one —
   * so a test driving the run through several states can react the instant
   * it *observes* each one, rather than guessing at wall-clock delays. A
   * `setTimeout`-scheduled update racing the route's own poll loop is
   * exactly the kind of interaction that's fine in production (nothing here
   * needs the two to interleave on any particular schedule) but makes a poor
   * test: fixed delays either wait needlessly or, on a slower CI runner,
   * race the assertion.
   */
  async function driveEvents(
    res: Response,
    onEvent: (data: unknown) => Promise<void> | void,
  ): Promise<unknown[]> {
    const reader = res.body!.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    const events: unknown[] = [];
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      let boundary: number;
      while ((boundary = buffer.indexOf("\n\n")) !== -1) {
        const frame = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary + 2);
        if (!frame.startsWith("data: ")) continue;
        const data = JSON.parse(frame.slice("data: ".length));
        events.push(data);
        await onEvent(data);
      }
    }
    return events;
  }

  async function stream(runId: string) {
    const { GET } = await import("./route.js");
    return GET(new Request(`http://localhost/api/runs/${runId}/stream`), {
      params: Promise.resolve({ id: runId }),
    });
  }

  it("sends one event and ends promptly for a run already in a terminal state", async () => {
    const runId = await runIn("SUCCEEDED");

    const started = Date.now();
    const res = await stream(runId);
    const events = await readEvents(res);
    const elapsed = Date.now() - started;

    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({ id: runId, status: "SUCCEEDED" });
    // Well under the 8s segment cap — it should close as soon as it sees the
    // terminal status, not idle out the rest of the segment.
    expect(elapsed).toBeLessThan(3_000);
  });

  it("pushes each distinct state as the run changes mid-stream", async () => {
    const runId = await runIn("QUEUED");
    const res = await stream(runId);

    const events = await driveEvents(res, async (data) => {
      const status = (data as { status?: string }).status;
      // React the instant each state is actually observed, rather than
      // guessing when the route's poll loop will next check — see
      // `driveEvents`.
      if (status === "QUEUED") {
        await prisma.run.update({ where: { id: runId }, data: { status: "RUNNING" } });
      } else if (status === "RUNNING") {
        await prisma.run.update({ where: { id: runId }, data: { status: "SUCCEEDED" } });
      }
    });

    const statuses = events.map((e) => (e as { status: string }).status);
    expect(statuses).toEqual(["QUEUED", "RUNNING", "SUCCEEDED"]);
  }, 10_000);

  it("sends notFound and ends for a run outside the caller's org", async () => {
    const otherOrg = await prisma.organization.create({
      data: { name: "Other", slug: `${slug}-other` },
    });
    const runId = await runIn("SUCCEEDED");
    await prisma.run.update({ where: { id: runId }, data: { orgId: otherOrg.id } });

    const events = await readEvents(await stream(runId));

    expect(events).toEqual([{ notFound: true }]);
    await prisma.organization.delete({ where: { id: otherOrg.id } }).catch(() => undefined);
  });
});
