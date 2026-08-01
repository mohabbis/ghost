import { describe, expect, it } from "vitest";
import { runWorkflowJobId, QUEUE_NAMES } from "./queue.js";

describe("runWorkflowJobId", () => {
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
