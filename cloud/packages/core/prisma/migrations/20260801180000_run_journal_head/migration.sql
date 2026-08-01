-- Persist the expected journal tail outside RunEvent. This closes the case
-- where deleting a complete suffix leaves all surviving hash links valid.
ALTER TABLE "Run" ADD COLUMN "journalHead" TEXT;

-- Existing journals predate the invariant, so initialize them from their
-- current tail. Future appends update this column transactionally.
UPDATE "Run" AS r
SET "journalHead" = tail.hash
FROM (
  SELECT DISTINCT ON ("runId") "runId", hash
  FROM "RunEvent"
  ORDER BY "runId", seq DESC
) AS tail
WHERE r.id = tail."runId";
