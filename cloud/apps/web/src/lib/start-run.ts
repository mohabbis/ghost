import { prisma } from "@/lib/db";
import { enqueueRunWorkflow } from "@/lib/queue";

/**
 * Create a run and hand it to the queue, or leave no ambiguity that it never
 * started.
 *
 * The two operations cannot be one transaction — the row must exist before
 * there is an id to enqueue, and Redis is not in the database's transaction.
 * So the failure to handle is: the `Run` commits and the enqueue then throws
 * (Redis down, wrong URL, connection dropped).
 *
 * Left alone, that leaves a row in QUEUED which no worker will ever see. It is
 * indistinguishable in the UI from a run waiting its turn behind a concurrency
 * cap, so it reads as "starting soon" forever. Observed for real: Redis died
 * mid-session, the route 500'd, and a QUEUED run sat there looking pending
 * while nothing was going to happen.
 *
 * Marking it FAILED with a stated reason is the honest outcome. The attempt is
 * preserved (deleting the row would hide that someone pressed Run), and the
 * status says plainly that no steps executed. No journal entries are written,
 * because nothing ran — a `run.started` here would be a lie in the audit chain.
 */
export type StartRunResult =
  | { ok: true; runId: string }
  | { ok: false; runId: string; error: string };

export async function startRun(input: {
  orgId: string;
  workflowVersionId: string;
  triggeredById?: string | null;
}): Promise<StartRunResult> {
  const run = await prisma.run.create({
    data: {
      orgId: input.orgId,
      workflowVersionId: input.workflowVersionId,
      triggeredById: input.triggeredById ?? null,
      status: "QUEUED",
    },
  });

  try {
    await enqueueRunWorkflow({ runId: run.id, orgId: input.orgId });
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err);
    // Best-effort: if this write also fails the row stays QUEUED, which is the
    // old behaviour rather than a new one. Do not let it mask the real cause.
    await prisma.run
      .update({
        where: { id: run.id },
        data: { status: "FAILED", error: `could not be queued: ${reason}`, endedAt: new Date() },
      })
      .catch(() => undefined);
    return { ok: false, runId: run.id, error: `could not be queued: ${reason}` };
  }

  return { ok: true, runId: run.id };
}
