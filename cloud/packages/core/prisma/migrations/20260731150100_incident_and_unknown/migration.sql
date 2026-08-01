-- Enum additions live in their own migration: ALTER TYPE ... ADD VALUE cannot
-- run in the same transaction as statements that use the new value.

-- A failed or unresumable run waits for a human instead of dying.
ALTER TYPE "RunStatus" ADD VALUE 'INCIDENT';

-- An at-most-once step whose outcome the engine cannot determine.
ALTER TYPE "StepStatus" ADD VALUE 'UNKNOWN';
