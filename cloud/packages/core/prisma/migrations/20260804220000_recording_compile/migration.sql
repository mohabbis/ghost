-- CreateEnum
CREATE TYPE "RecordingCompileStatus" AS ENUM ('NONE', 'RUNNING', 'READY', 'FAILED');

-- AlterTable
ALTER TABLE "Recording"
  ADD COLUMN "rawTraceFilename" TEXT,
  ADD COLUMN "compileStatus" "RecordingCompileStatus" NOT NULL DEFAULT 'NONE',
  ADD COLUMN "harnessResponseId" TEXT,
  ADD COLUMN "harnessSessionId" TEXT,
  ADD COLUMN "compiledSteps" JSONB,
  ADD COLUMN "compileNotes" TEXT,
  ADD COLUMN "compileError" TEXT,
  ADD COLUMN "workflowId" TEXT;

-- CreateIndex
CREATE INDEX "Recording_workflowId_idx" ON "Recording"("workflowId");

-- AddForeignKey
ALTER TABLE "Recording" ADD CONSTRAINT "Recording_workflowId_fkey" FOREIGN KEY ("workflowId") REFERENCES "Workflow"("id") ON DELETE SET NULL ON UPDATE CASCADE;
