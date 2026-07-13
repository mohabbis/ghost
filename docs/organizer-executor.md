# Organizer Executor, Audit Log & Undo Journal

The executor is the **Execution / Audit / Undo** end of the trust pipeline — the
first organizer code that mutates the filesystem. It applies an *already
approved* plan and nothing else:

```text
Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
         (planner, read-only)          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                                        executor + undo (this doc)
```

It lives in `src-tauri/src/organizer/{executor,undo}.rs` and writes its records
into the passive, serializable ledgers in `src-tauri/src/audit/`
(`audit_log`, `undo_journal`). The audit module holds **no filesystem logic**:
all disk mutation stays in `organizer`, while `audit` stays a neutral, inspectable
record. Like the planner, it is exposed through
`src-tauri/src/commands/organizer.rs`; the command registry documents the
resulting risk classes in `docs/command-registry.md`.

## Execution flow

`executor::execute_plan(plan, rules)` walks each `PlanAction` and, for every one:

1. **Re-checks policy.** It calls `policy::evaluate_with_attribution` again at
   execution time. Anything the engine now refuses is recorded as `Skipped` and
   never touched — even though it was in the plan (rules or disk state may have
   drifted). The evaluation also names the `FolderRule` that fired, which is
   carried onto the audit event.
2. **Verifies state.** A move/rename requires the source to still exist and the
   target to *not* exist. Ghost never silently overwrites (`AGENTS.md`
   non-negotiable): an occupied target is `Skipped`, not clobbered. The target
   parent folder must already exist from an explicit, policy-checked
   `CreateFolder` action; the move/rename step never creates missing parent
   folders implicitly because that would be an unaudited mutation.
3. **Prepares undo before mutating.** The inverse op (`UndoOp::RemoveFolder` for
   a created folder, `UndoOp::Restore` for a move/rename) is constructed before
   the filesystem call but not yet committed to the `UndoJournal`.
4. **Applies, verifies the result, commits undo, and audits.** The inverse is
   recorded only after the filesystem postcondition is verified, so failed or
   skipped actions do not create false rollback entries. Each action produces exactly
   one `AuditEvent` (`Applied` / `Skipped` / `Failed`), tagged with the rule
   that fired (`rule_path`) and — for actions that ran — how it was authorized
   (`provenance`: `Automated` under an `automate` rule, `UserApproved`
   otherwise). Both attribution fields are optional and serde-defaulted, so
   audit logs persisted before they existed still deserialize.

Only the file-organization capabilities the planner emits (`CreateFolder`,
`MoveFile`, `RenameFile`) are executable; any other capability is `Skipped`
rather than risked. The result is an `ExecutionReport { applied, skipped,
failed, audit, undo }`.

`execute_plan` itself holds this report in memory until the whole plan
finishes — fine for callers that persist it once at the end, but see
"Crash recovery" below for the durable path `organizer_execute` actually uses.

## Crash recovery (write-ahead durability)

`execute_plan` builds its `ExecutionReport` in memory across the whole plan;
a naive caller that persists it only once, after the loop returns, loses the
*entire* run's undo journal if the process dies mid-run — even though some
files were already moved on disk and successfully verified.

`organizer_execute` avoids this with a write-ahead sequence, using
`executor::execute_plan_with_progress` (identical to `execute_plan`, but
invokes a callback with the report-so-far after every action) together with
three `storage::executions` functions:

1. **`begin_execution`** inserts a row *before* the executor touches the
   filesystem — empty, unsealed, `finished = 0`.
2. **`update_execution_progress`** overwrites that row's content after every
   action (the `execute_plan_with_progress` callback). A crash between two
   actions loses at most the most recent snapshot write, never the whole run.
3. **`finish_execution`** writes the final content, computes the real
   tamper-evident seal (excluding the row's own pre-seal empty hash from the
   chain-tip lookup), and sets `finished = 1`.

If the app crashes between (1)/(2) and (3), the row survives with
`finished = 0` and an accurate (if not final) undo journal for whatever had
actually been applied. `organizer_check_unfinished_run` finds it on the next
Organizer view load (`find_unfinished_execution`: the newest row with
`finished = 0`), and the frontend offers the user the same two, honest
options: **undo** what was applied (`organizer_undo` — it already works on
any stored execution by id, unfinished ones included — which also marks the
row resolved) or **dismiss** it as intentional
(`organizer_dismiss_unfinished_run`, a bare `mark_execution_finished`). There
is no "resume the rest of the plan" option: the plan itself was never
persisted, only what the executor actually did, so re-running Organizer to
plan and apply the remainder fresh is the correct next step either way.

Rows written before this existed (V6 migration) default to `finished = 1` —
they predate write-ahead durability entirely, so by definition they already
ran to completion one way or another; none are mistaken for an interrupted run.

## Export

`AuditLog::to_csv()` renders a run's log as RFC-4180 CSV (one row per event:
timestamp, action, path(s), outcome, detail, rule, provenance). The
`organizer_export_audit` command returns either that CSV or the pretty-printed
JSON of the audit log; it reads the stored run and returns text, writing nothing
itself (the frontend saves the file the user picks). This is Ghost's version of
an exportable "what the machine did, and under which rule" record.

Every export format (`csv`, `compliance`, `signed`, and `json`) runs paths and
skip/fail detail text through `audit::pii::mask` first (`AuditLog::masked()`
for `json`), redacting SSN-, card-, email-, and phone-shaped substrings — the
Organizer's filing preset routes tax/financial documents whose *filenames*
can carry that kind of PII (`docs/filing-profiles.md`). The log that's
actually persisted and hash-chained (`storage::executions`) and the one undo
replays against are never touched, so redaction only affects what leaves the
app as a portable file — it can't desync a run's tamper-evident seal.

## Undo

`undo::revert(journal)` replays the `UndoJournal` **in reverse order** (newest
step first), so files are moved out of a destination folder before that folder's
`RemoveFolder` runs. Reversal keeps the executor's safety stance:

- **Never overwrites** — a `Restore` whose origin is now occupied is skipped.
- **Never recursively deletes** — `RemoveFolder` uses `remove_dir`, which only
  succeeds on an empty directory; a folder a user refilled after execution is
  left in place (counted as skipped). Returns an `UndoReport { reverted,
  skipped, failed }`.

## Invariants

- **No deletes.** The executor never removes files; undo only removes folders it
  created, and only when empty.
- **No silent overwrite.** Targets are re-checked at execution and at undo.
- **No implicit folder mutation.** Missing target parents skip the file instead
  of creating folders that were not separately planned, approved, audited, and
  journaled.
- **Undo describes applied state.** Every reversible op prepares its inverse before
  it runs, then journals that inverse only after successful postcondition verification.
- **Partial failure is recoverable.** Each action is independent; a failure on
  one leaves prior successful actions and their undo entries intact without adding
  undo entries for failed/skipped work.
- **Deterministic & policy-gated.** Denied actions never reach the filesystem.
- **TOCTOU / inode identity.** `organizer/file_identity.rs` captures dev/ino (or
  Windows file index) at scan time on each `PlanAction`. `relocate` re-stats the
  source before `rename` and skips when identity drifted or the source is a symlink.

## Status

Implemented: `audit::audit_log`, `audit::undo_journal`, `organizer::executor`,
`organizer::undo`, with unit tests covering happy-path application, policy-denied
skips, overwrite refusal, missing-source skips, unsupported-capability rejection,
missing-target-parent skips without implicit folder creation, a full
execute→undo round-trip back to the original tree, and folder-preservation when a
user refills a created folder. The executor and undo runner are wired to Tauri
through `organizer_execute` and `organizer_undo`; the frontend still owns the
explicit review/approval affordance before calling execution.

Also implemented: write-ahead durability (`begin_execution` /
`update_execution_progress` / `finish_execution` / `find_unfinished_execution` /
`mark_execution_finished` in `storage::executions`), with unit tests covering a
simulated mid-run crash (the undo journal for already-applied steps survives),
correct chain-tip linking when finalizing a WAL-started run, and the
resolve-without-resealing path. Wired to Tauri through
`organizer_check_unfinished_run` and `organizer_dismiss_unfinished_run`; the
frontend checks on Organizer view load and offers undo-or-dismiss via a banner.
