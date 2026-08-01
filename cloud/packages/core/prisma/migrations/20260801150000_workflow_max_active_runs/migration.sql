-- Per-workflow concurrency cap (Airflow's `max_active_runs`).
--
-- Nullable with no default, so every existing workflow keeps today's behaviour
-- (uncapped) rather than silently acquiring a limit on deploy. A cap is
-- something an operator opts into for a system that cannot take the load.
ALTER TABLE "Workflow" ADD COLUMN "maxActiveRuns" INTEGER;

-- The admission query counts a workflow's in-flight runs on every start and
-- resume: Run -> WorkflowVersion -> Workflow, filtered by status.
CREATE INDEX "Run_workflowVersionId_status_idx" ON "Run"("workflowVersionId", "status");
