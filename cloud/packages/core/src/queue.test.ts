import { describe, expect, it } from "vitest";
import { compensateRunJobId, runWorkflowJobId, QUEUE_NAMES } from "./queue.js";

describe("runWorkflowJobId", () => {
  it("produces ids BullMQ will accept", () => {
    // BullMQ validates custom ids: it splits on `:` for repeatable-job
    // compatibility and throws unless the result is one part or three. The
    // previous format was `run:<id>:<from>:<token>` — four parts — so *every*
    // enqueue that carried a resume token threw before reaching Redis. That is
    // every incident retry, every incident skip, and the cleanup enqueue on
    // cancel and on a rejected approval. It went unnoticed because the only
    // path exercised against a real queue never passed a token.
    //
    // Asserted as the constraint rather than the exact string, so changing the
    // format stays free as long as it stays legal.
    const ids = [
      runWorkflowJobId("clxrun123"),
      runWorkflowJobId("clxrun123", 2),
      runWorkflowJobId("clxrun123", 2, "incident-3"),
      runWorkflowJobId("clxrun123", undefined, "cleanup-1700000000"),
      compensateRunJobId("clxrun123"),
    ];
    for (const id of ids) {
      const parts = id.split(":").length;
      expect(parts === 1 || parts === 3, `illegal job id: ${id}`).toBe(true);
    }
  });

  it("keeps a run and its reversal apart", () => {
    expect(compensateRunJobId("r1")).not.toBe(runWorkflowJobId("r1"));
  });

  it("separates a resumed compensation from the request that started it", () => {
    // A reversal that gates, is approved, and resumes goes through the same
    // queue twice. Sharing an id means BullMQ drops the resume — completed jobs
    // are retained — and the run sits in COMPENSATING forever with the API
    // reporting success. This is the assertion the producer must satisfy; it
    // was passing a token that the producer then discarded.
    expect(compensateRunJobId("r1", "gate-1-123")).not.toBe(compensateRunJobId("r1"));
    expect(compensateRunJobId("r1", "gate-1-123")).not.toBe(
      compensateRunJobId("r1", "gate-1-456"),
    );
  });

  it("collapses duplicate submissions from the same resume point", () => {
    // A double-clicked Run button or a double-submitted approval must not race
    // two workers over one run.
    expect(runWorkflowJobId("r1")).toBe(runWorkflowJobId("r1"));
    expect(runWorkflowJobId("r1", 3)).toBe(runWorkflowJobId("r1", 3));
  });

  it("separates different runs and different resume points", () => {
    expect(runWorkflowJobId("r1")).not.toBe(runWorkflowJobId("r2"));
    expect(runWorkflowJobId("r1", 3)).not.toBe(runWorkflowJobId("r1", 4));
    expect(runWorkflowJobId("r1")).not.toBe(runWorkflowJobId("r1", 0));
  });

  it("separates a deliberate re-drive from the same step index", () => {
    // The bug this guards: a run that gated at step 3, was approved, failed,
    // and is then retried from an incident resumes from step 3 twice. Sharing
    // an id meant BullMQ silently dropped the second add — because completed
    // jobs are retained — so the API reported success, the run sat in QUEUED,
    // and no worker ever picked it up.
    const approvalResume = runWorkflowJobId("r1", 3);
    const incidentRetry = runWorkflowJobId("r1", 3, "incident-42");
    expect(incidentRetry).not.toBe(approvalResume);

    // ...and two distinct incident retries stay distinct.
    expect(runWorkflowJobId("r1", 3, "incident-43")).not.toBe(incidentRetry);
  });

  it("names the queues it claims to", () => {
    expect(QUEUE_NAMES.runWorkflow).toBe("run-workflow");
  });
});
