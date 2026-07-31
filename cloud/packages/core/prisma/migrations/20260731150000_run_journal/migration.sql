-- Durable execution: per-run hash-chained journal, run leases, session capture,
-- and the constraints that stop the audit chain forking under concurrency.

-- ---------------------------------------------------------------------------
-- RunEvent: the append-only execution journal.
-- ---------------------------------------------------------------------------
CREATE TABLE "RunEvent" (
    "id"        TEXT NOT NULL,
    "runId"     TEXT NOT NULL,
    "seq"       INTEGER NOT NULL,
    "stepIndex" INTEGER,
    "type"      TEXT NOT NULL,
    "payload"   JSONB,
    "prevHash"  TEXT,
    "hash"      TEXT NOT NULL,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "RunEvent_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX "RunEvent_runId_seq_key" ON "RunEvent"("runId", "seq");
CREATE INDEX "RunEvent_runId_seq_idx" ON "RunEvent"("runId", "seq");

ALTER TABLE "RunEvent" ADD CONSTRAINT "RunEvent_runId_fkey"
  FOREIGN KEY ("runId") REFERENCES "Run"("id") ON DELETE CASCADE ON UPDATE CASCADE;

-- ---------------------------------------------------------------------------
-- AuditEvent.seq: deterministic ordering + a constraint that makes a lost
-- append race fail loudly instead of forking the chain.
-- Backfill matches the ordering the existing chain was built with.
-- ---------------------------------------------------------------------------
ALTER TABLE "AuditEvent" ADD COLUMN "seq" INTEGER;

UPDATE "AuditEvent" a
SET "seq" = s.rn
FROM (
  SELECT "id", row_number() OVER (PARTITION BY "orgId" ORDER BY "createdAt", "id") AS rn
  FROM "AuditEvent"
) s
WHERE a."id" = s."id";

ALTER TABLE "AuditEvent" ALTER COLUMN "seq" SET NOT NULL;

CREATE UNIQUE INDEX "AuditEvent_orgId_seq_key" ON "AuditEvent"("orgId", "seq");
CREATE INDEX "AuditEvent_orgId_seq_idx" ON "AuditEvent"("orgId", "seq");

-- ---------------------------------------------------------------------------
-- Run: lease + captured session pointer.
-- ---------------------------------------------------------------------------
ALTER TABLE "Run"
  ADD COLUMN "leaseOwner"     TEXT,
  ADD COLUMN "leaseExpiresAt" TIMESTAMP(3),
  ADD COLUMN "attempt"        INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN "sessionKey"     TEXT,
  ADD COLUMN "sessionUrl"     TEXT;

CREATE INDEX "Run_status_leaseExpiresAt_idx" ON "Run"("status", "leaseExpiresAt");

-- ---------------------------------------------------------------------------
-- RunStep: attempt counter + named outputs for the expression resolver.
-- ---------------------------------------------------------------------------
ALTER TABLE "RunStep"
  ADD COLUMN "attempt" INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN "output"  JSONB;

-- ---------------------------------------------------------------------------
-- Approval: one row per gated step, plus expiry.
-- Collapse any pre-existing duplicates (keep the earliest) before constraining.
-- ---------------------------------------------------------------------------
ALTER TABLE "Approval" ADD COLUMN "expiresAt" TIMESTAMP(3);

DELETE FROM "Approval" a
USING "Approval" b
WHERE a."runId" = b."runId"
  AND a."stepIndex" = b."stepIndex"
  AND a."id" > b."id";

CREATE UNIQUE INDEX "Approval_runId_stepIndex_key" ON "Approval"("runId", "stepIndex");
