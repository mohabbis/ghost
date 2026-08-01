-- Enum additions live alone: ALTER TYPE ... ADD VALUE cannot share a
-- transaction with statements that use the new value.

-- Reversing a run's completed side effects (BPMN compensation).
ALTER TYPE "RunStatus" ADD VALUE 'COMPENSATING';
-- Reversal finished. Distinct from SUCCEEDED and from never having run.
ALTER TYPE "RunStatus" ADD VALUE 'COMPENSATED';
