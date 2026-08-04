-- Persist the expected org audit chain tail outside AuditEvent, mirroring
-- Run.journalHead. This closes the case where deleting a complete suffix of
-- an org's AuditEvent rows leaves all surviving hash links valid.
ALTER TABLE "Organization" ADD COLUMN "auditChainHead" TEXT;

-- Existing org chains predate the invariant, so initialize them from their
-- current tail. Future appends update this column transactionally. An org
-- with no AuditEvent rows yet is left NULL, matching a fresh chain's expected
-- head of "no events".
UPDATE "Organization" AS o
SET "auditChainHead" = tail.hash
FROM (
  SELECT DISTINCT ON ("orgId") "orgId", hash
  FROM "AuditEvent"
  ORDER BY "orgId", seq DESC
) AS tail
WHERE o.id = tail."orgId";
