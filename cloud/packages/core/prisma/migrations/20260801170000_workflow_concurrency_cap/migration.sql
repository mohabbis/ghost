-- Per-workflow concurrency cap (Airflow's `max_active_runs`).
--
-- Nullable with no default, so every existing workflow keeps today's behaviour
-- (uncapped) rather than silently acquiring a limit on deploy. A cap is
-- something an operator opts into for a system that cannot take the load.
ALTER TABLE "Workflow" ADD COLUMN "maxActiveRuns" INTEGER;

-- Explicit slot ownership, rather than inferring it from `Run.status`.
--
-- Status moves for reasons unrelated to whether a run is still touching the
-- customer's system: a cancel marks a run CANCELED while the worker is
-- deliberately letting the current action finish, and an approved run passes
-- through QUEUED on its way back to RUNNING. Counting by status leaves both
-- windows open for a second run to be admitted alongside one still executing.
ALTER TABLE "Run" ADD COLUMN "slotHeldAt" TIMESTAMP(3);

-- Admission counts a workflow's slot holders on every start and resume.
CREATE INDEX "Run_workflowVersionId_status_idx" ON "Run"("workflowVersionId", "status");
CREATE INDEX "Run_workflowVersionId_slotHeldAt_idx" ON "Run"("workflowVersionId", "slotHeldAt");
