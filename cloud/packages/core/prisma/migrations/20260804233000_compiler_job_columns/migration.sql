-- Rename the compile-handle columns off a vendor's vocabulary and onto the
-- role they play. The compiler behind them is chosen at runtime, is optional,
-- and is authoring tooling rather than part of the execution runtime — see
-- cloud/docs/DEPLOY.md. Naming the columns after one implementation put a
-- replaceable dependency into the core data model.
--
-- A rename rather than add-and-backfill: no deployment of Ghost exists yet, so
-- there is no live reader of the old names to keep working.
ALTER TABLE "Recording" RENAME COLUMN "harnessResponseId" TO "compileJobId";
ALTER TABLE "Recording" RENAME COLUMN "harnessSessionId" TO "compileSessionId";
