-- AlterTable
ALTER TABLE "Run" ADD COLUMN     "artifactsPurgedAt" TIMESTAMP(3);

-- CreateIndex
CREATE INDEX "Run_endedAt_artifactsPurgedAt_idx" ON "Run"("endedAt", "artifactsPurgedAt");
