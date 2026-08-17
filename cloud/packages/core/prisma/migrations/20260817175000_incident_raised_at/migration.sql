-- Track when a run entered INCIDENT, so the exception queue can order by how
-- long something has actually been parked rather than by how old the run is.
--
-- Backfilled to NULL, not to `createdAt`: guessing would assert that a run
-- created last week has been waiting since last week, which is the exact
-- misreading this column exists to remove. The queue falls back to `createdAt`
-- for these pre-existing rows and says so, rather than inventing a timestamp.

-- AlterTable
ALTER TABLE "Run" ADD COLUMN     "incidentRaisedAt" TIMESTAMP(3);

-- The previous index was on (orgId, status, createdAt); the queue no longer
-- orders on createdAt, so it is replaced rather than added to.
-- DropIndex
DROP INDEX IF EXISTS "Run_orgId_status_createdAt_idx";

-- CreateIndex
CREATE INDEX "Run_orgId_status_incidentRaisedAt_idx" ON "Run"("orgId", "status", "incidentRaisedAt");
