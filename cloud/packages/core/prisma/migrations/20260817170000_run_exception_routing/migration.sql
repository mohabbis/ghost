-- Exception routing: classify an incident when it is raised, and let a human own it.
--
-- Both columns are nullable with no default: every existing row is either not an
-- incident (so they are correctly null) or is a pre-existing incident that was
-- never classified. The queue renders an unclassified incident as UNKNOWN rather
-- than backfilling a guess, because the reason string it would have to guess from
-- may since have been overwritten by a later failure on the same run.

-- AlterTable
ALTER TABLE "Run" ADD COLUMN     "incidentKind" TEXT,
ADD COLUMN     "incidentAssigneeId" TEXT;

-- CreateIndex
CREATE INDEX "Run_orgId_status_createdAt_idx" ON "Run"("orgId", "status", "createdAt");

-- CreateIndex
CREATE INDEX "Run_incidentAssigneeId_idx" ON "Run"("incidentAssigneeId");

-- AddForeignKey
-- ON DELETE SET NULL: removing a person from the org returns their open
-- exceptions to the unassigned queue. It must never cascade a run away.
ALTER TABLE "Run" ADD CONSTRAINT "Run_incidentAssigneeId_fkey" FOREIGN KEY ("incidentAssigneeId") REFERENCES "User"("id") ON DELETE SET NULL ON UPDATE CASCADE;
