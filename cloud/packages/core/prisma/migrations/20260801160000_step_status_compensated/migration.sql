-- A reversed step is not a skipped step.
--
-- `SKIPPED` was being written over the forward RunStep after its reversal
-- succeeded, which made the timeline and the run-detail API report that the
-- original action never executed — when it executed and is the reason there was
-- anything to undo. Additive: no existing row changes meaning.
ALTER TYPE "StepStatus" ADD VALUE 'COMPENSATED';
