/**
 * The run journal's event vocabulary.
 *
 * A run's execution position is a *fold over these events*, not a stored
 * cursor. That is the whole point: a step with a recorded terminal success is
 * structurally unreachable on resume, so a crash mid-run cannot re-fire an
 * already-approved payment.
 *
 * Payloads are audit surface. They never carry secrets — no `fill` values, no
 * storageState, no cookies. Where a blob matters, the payload records its
 * digest, so the chain commits to something provable rather than readable.
 */

export const RUN_EVENT_TYPES = {
  /** The run began (first job only; a resume records `run.resumed`). */
  runStarted: "run.started",
  /** A job picked the run back up after a halt or a crash. */
  runResumed: "run.resumed",
  runSucceeded: "run.succeeded",
  runFailed: "run.failed",
  runCanceled: "run.canceled",

  /** A step is about to act. Written BEFORE the effect, so the crash window is recorded. */
  stepStarted: "step.started",
  stepSucceeded: "step.succeeded",
  stepFailed: "step.failed",
  /** A human resolved an incident by skipping this step. */
  stepSkipped: "step.skipped",
  /**
   * One attempt failed and another follows, within a single execution of the
   * step. Makes retry counts auditable.
   *
   * Deliberately does NOT change what the journal fold believes about the
   * step's state. A verification retry is appended *after* the action has
   * already run, so treating it as "no longer in flight" would let a crash
   * mid-retry make an already-executed sensitive step look un-executed — and
   * re-run it.
   */
  stepRetried: "step.retried",
  /**
   * A human resolved an incident by asking for the step to run again. This is
   * the only event that clears a recorded failure, and it is separate from
   * `step.retried` precisely so the two can never be confused.
   */
  stepRetryRequested: "step.retry_requested",
  /**
   * An at-most-once step started and never recorded a terminal outcome. The
   * engine cannot tell whether the effect landed; a human must decide.
   */
  stepOutcomeUnknown: "step.outcome_unknown",

  /** The run halted for approval. */
  gateOpened: "gate.opened",
  /** A human approved or rejected. This is the record of *who authorized what*. */
  gateResolved: "gate.resolved",

  /** Browser storageState was captured at a halt (payload: key + digest, never contents). */
  sessionCaptured: "session.captured",
  sessionRestored: "session.restored",
  /** Page state could not be provably rebuilt; the run stops rather than guessing. */
  sessionRestoreFailed: "session.restore_failed",
} as const;

export type RunEventType = (typeof RUN_EVENT_TYPES)[keyof typeof RUN_EVENT_TYPES];

/** Event types that mark a step as durably complete — never re-execute these. */
export const TERMINAL_SUCCESS_EVENTS: ReadonlySet<string> = new Set([
  RUN_EVENT_TYPES.stepSucceeded,
  RUN_EVENT_TYPES.stepSkipped,
]);

/** Event types that mark a step as durably failed. */
export const TERMINAL_FAILURE_EVENTS: ReadonlySet<string> = new Set([
  RUN_EVENT_TYPES.stepFailed,
  RUN_EVENT_TYPES.stepOutcomeUnknown,
]);
