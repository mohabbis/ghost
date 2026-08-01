-- Persist the expected tail outside RunEvent so deleting a complete journal
-- suffix cannot leave an internally-valid prefix eligible to drive execution.
ALTER TABLE "Run"
  ADD COLUMN "journalHead" TEXT,
  ADD COLUMN "journalSeq" INTEGER NOT NULL DEFAULT 0;

-- Existing journals predate the expected-tail columns. Backfill each run from
-- its highest sequence row without rewriting or deleting any journal data.
UPDATE "Run" AS r
SET
  "journalHead" = tail.hash,
  "journalSeq" = tail.seq
FROM (
  SELECT DISTINCT ON ("runId") "runId", hash, seq
  FROM "RunEvent"
  ORDER BY "runId", seq DESC
) AS tail
WHERE r.id = tail."runId";
