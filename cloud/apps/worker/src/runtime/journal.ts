import {
  RUN_EVENT_TYPES,
  TERMINAL_SUCCESS_EVENTS,
  TERMINAL_FAILURE_EVENTS,
} from "@ghost/core/run-events";

/**
 * Fold the run journal into the facts the state machine needs.
 *
 * Pure — no Prisma, no Playwright — so the durability rules are exhaustively
 * unit-testable. The shell loads `RunEvent` rows and calls this; nothing else
 * decides what has already happened.
 */

export interface RunJournal {
  /** Indices with a recorded terminal success. These are never re-executed. */
  completed: ReadonlySet<number>;
  /** Indices with a recorded terminal failure. */
  failed: ReadonlySet<number>;
  /**
   * Indices that recorded `step.started` and no terminal outcome — the crash
   * window. The side effect may or may not have reached the outside world.
   */
  inFlight: ReadonlySet<number>;
  /** Values captured by `extract` steps, keyed by step id then value name. */
  outputs: Readonly<Record<string, Readonly<Record<string, string>>>>;
  /** Per-index attempt counts, for retry budgeting across a resume. */
  attempts: ReadonlyMap<number, number>;
}

export interface JournalEventRow {
  seq: number;
  type: string;
  stepIndex: number | null;
  payload: unknown;
}

export function emptyJournal(): RunJournal {
  return {
    completed: new Set(),
    failed: new Set(),
    inFlight: new Set(),
    outputs: {},
    attempts: new Map(),
  };
}

/** Build the journal from `RunEvent` rows, which must be ordered by `seq`. */
export function journalFromEvents(rows: readonly JournalEventRow[]): RunJournal {
  const completed = new Set<number>();
  const failed = new Set<number>();
  const inFlight = new Set<number>();
  const outputs: Record<string, Record<string, string>> = {};
  const attempts = new Map<number, number>();

  for (const row of rows) {
    const i = row.stepIndex;

    if (row.type === RUN_EVENT_TYPES.stepStarted && i !== null) {
      inFlight.add(i);
      attempts.set(i, (attempts.get(i) ?? 0) + 1);
      continue;
    }

    if (row.type === RUN_EVENT_TYPES.stepRetried && i !== null) {
      // A retry supersedes the recorded failure. Append-only: the original
      // `step.failed` stays in the chain as history, but the fold stops
      // treating the step as terminally failed so it can run again. This is how
      // a human resolving an incident un-sticks a run without rewriting it.
      failed.delete(i);
      inFlight.delete(i);
      continue;
    }

    if (TERMINAL_SUCCESS_EVENTS.has(row.type) && i !== null) {
      completed.add(i);
      failed.delete(i);
      inFlight.delete(i);
      const captured = extractOutputs(row.payload);
      if (captured) outputs[captured.stepId] = { ...outputs[captured.stepId], ...captured.values };
      continue;
    }

    if (TERMINAL_FAILURE_EVENTS.has(row.type) && i !== null) {
      failed.add(i);
      inFlight.delete(i);
      continue;
    }
  }

  return { completed, failed, inFlight, outputs, attempts };
}

function extractOutputs(
  payload: unknown,
): { stepId: string; values: Record<string, string> } | null {
  if (!payload || typeof payload !== "object") return null;
  const p = payload as { stepId?: unknown; outputs?: unknown };
  if (typeof p.stepId !== "string" || !p.outputs || typeof p.outputs !== "object") return null;

  const values: Record<string, string> = {};
  for (const [k, v] of Object.entries(p.outputs as Record<string, unknown>)) {
    if (typeof v === "string") values[k] = v;
  }
  return Object.keys(values).length > 0 ? { stepId: p.stepId, values } : null;
}

/**
 * Backward compatibility for runs that predate the journal.
 *
 * Without this, deploying durable execution would make every in-flight run
 * look like "nothing has executed" and re-run it from index 0 — triggering the
 * exact double-submit the journal exists to prevent. Keyed on the *absence* of
 * events rather than on a date, so it ages out on its own.
 *
 * Such runs have no captured session, so restoration refuses to guess and they
 * fail loudly instead. A handful of runs needing a manual restart is the right
 * trade against any chance of a re-fired payment.
 */
export function journalFromLegacyRunSteps(
  steps: readonly { index: number; status: string }[],
): RunJournal {
  const completed = new Set<number>();
  const failed = new Set<number>();
  const inFlight = new Set<number>();

  for (const s of steps) {
    if (s.status === "SUCCEEDED" || s.status === "SKIPPED") completed.add(s.index);
    else if (s.status === "FAILED" || s.status === "UNKNOWN") failed.add(s.index);
    else if (s.status === "RUNNING") inFlight.add(s.index);
  }

  return { completed, failed, inFlight, outputs: {}, attempts: new Map() };
}
