-- Approvals now belong to a direction of the run. Without this, gating the
-- reversal of step N would collide with the forward approval for step N.

CREATE TYPE "ApprovalPhase" AS ENUM ('FORWARD', 'COMPENSATION');

ALTER TABLE "Approval"
  ADD COLUMN "phase" "ApprovalPhase" NOT NULL DEFAULT 'FORWARD';

-- Widen the uniqueness to include the phase.
DROP INDEX IF EXISTS "Approval_runId_stepIndex_key";
CREATE UNIQUE INDEX "Approval_runId_stepIndex_phase_key"
  ON "Approval"("runId", "stepIndex", "phase");
