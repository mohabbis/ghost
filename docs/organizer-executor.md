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
   non-negotiable): an occupied target is `Skipped`, not clobbered.
3. **Writes undo before mutating.** The inverse op (`UndoOp::RemoveFolder` for a
   created folder, `UndoOp::Restore` for a move/rename) is recorded in the
   `UndoJournal` *before* the filesystem call — trust invariant 8.
4. **Applies, verifies the result, and audits.** Each action produces exactly
   one `AuditEvent` (`Applied` / `Skipped` / `Failed`), tagged with the rule
   that fired (`rule_path`) and — for actions that ran — how it was authorized
   (`provenance`: `Automated` under an `automate` rule, `UserApproved`
   otherwise). Both attribution fields are optional and serde-defaulted, so
   audit logs persisted before they existed still deserialize.

Only the file-organization capabilities the planner emits (`CreateFolder`,
`MoveFile`, `RenameFile`) are executable; any other capability is `Skipped`
rather than risked. The result is an `ExecutionReport { applied, skipped,
failed, audit, undo }`.

## Export

`AuditLog::to_csv()` renders a run's log as RFC-4180 CSV (one row per event:
timestamp, action, path(s), outcome, detail, rule, provenance). The
`organizer_export_audit` command returns either that CSV or the pretty-printed
JSON of the audit log; it reads the stored run and returns text, writing nothing
itself (the frontend saves the file the user picks). This is Ghost's version of
an exportable "what the machine did, and under which rule" record.

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
- **Undo before execute.** Every reversible op is journaled before it runs, so a
  partial failure is still reversible.
- **Partial failure is recoverable.** Each action is independent; a failure on
  one leaves the others (and their undo entries) intact.
- **Deterministic & policy-gated.** Denied actions never reach the filesystem.

## Status

Implemented: `audit::audit_log`, `audit::undo_journal`, `organizer::executor`,
`organizer::undo`, with unit tests covering happy-path application, policy-denied
skips, overwrite refusal, missing-source skips, unsupported-capability rejection,
a full execute→undo round-trip back to the original tree, and folder-preservation
when a user refills a created folder. The executor and undo runner are wired to
Tauri through `organizer_execute` and `organizer_undo`; the frontend still owns
the explicit review/approval affordance before calling execution.
