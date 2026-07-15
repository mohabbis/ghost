# Organizer Planner

The Organizer planner turns an approved Zone into a **reviewable plan that
mutates nothing**. It is the `Plan` (and `Policy`) step of the trust pipeline:

```text
Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
         ^^^^^^^^^^^^^^^^^
         this module; the executor performs Execution/Audit/Undo.
```

It lives in `src-tauri/src/organizer/` and performs **only read-only filesystem
access** (directory listing + `stat`, never file contents). It reuses the
`policy` engine for every decision and loads boundaries from `storage`.

## Flow

`planner::plan_zone(conn, zone_id)` loads the Zone and its `FolderRule`s via
`storage::zones`, then calls `plan_with_rules_and_options(zone_id, &rules,
rename_dated)`. `plan_with_rules(zone_id, &rules)` is also exposed directly so
the planner is testable without a database and defaults dated renaming off:

1. **Scan** every readable source folder (`scanner::scan`), bounded to
   `scanner::MAX_DEPTH`, de-duplicating files reachable from overlapping roots
   and sorting by path for deterministic output.
2. **Filter** system/ignored entries (`scanner::is_ignored`): dotfiles,
   `Thumbs.db`/`desktop.ini`/recycle-bin metadata, and temp/partial-download
   suffixes (`~`, `.tmp`, `.part`, `.crdownload`, …).
3. **Classify** each file deterministically (`classifier::classify`): extension
   first (mapped to a `Category`), enriched with filename keyword signals.
   Document-like files with `invoice`, `receipt`, or `statement` in the name
   route to `Invoices`, `Receipts`, or `Statements` at confidence `0.85`;
   otherwise document extensions route to `Documents` at confidence `0.95`.
   Unknown/absent extensions become `Category::Other` with low confidence. No
   AI, no IO. Each result carries a `confidence` in `0.0..=1.0` and a
   human-readable `reason`.
4. **Propose a target**: `<destination root>/<Category>/<safe name>`. The
   destination root is the first folder rule (ordered by path) granting both
   *create* and *move*. When the Zone has `rename_dated` enabled, the planner
   first prefixes the filename with the file's modified month (falling back to
   created time) as `YYYY-MM name`; existing `YYYY-MM ` or `YYYY-MM-` prefixes
   are left unchanged. The safe name then comes from `naming::safe_file_name`
   (cross-platform sanitization, no silent meaning change).
5. **Detect conflicts** (`conflict`): if the proposed target already exists on
   disk (`TargetExists`) or another planned file claims it (`DuplicateInPlan`),
   the planner picks a de-duplicated name (`name (2).ext`) via
   `naming::deduplicate` and **records the conflict on the action**. The
   Organizer never silently overwrites.
6. **Build capabilities**: `CreateFolder` (once per new destination folder),
   then `MoveFile` (different folder) or `RenameFile` (same folder, name change).
   Files already correctly placed and safely named are skipped
   (`SkipReason::AlreadyOrganized`).
7. **Evaluate** every capability through `policy::evaluate` and attach the
   resulting `PolicyDecision` to the action.
8. **Return** an `OrganizerPlan` — `actions` (each with decision, confidence,
   reason, optional conflict), `skipped` files, and a `PlanSummary` of counts.

## Output shape

- `PlanAction { capability, decision, confidence, reason, conflict }`
- `SkippedFile { path, reason }` where `reason` is `AlreadyOrganized` or
  `NoDestination`
- `PlanSummary { files_scanned, create_folder, move_file, rename_file,
  conflicts, low_confidence, denied, skipped }`

A Zone with no create+move rule yields a plan where every file is skipped with
`NoDestination` — deny-by-default, surfaced rather than hidden.

## Invariants

- **No mutation.** The planner only reads; a unit test asserts the directory
  tree is byte-for-byte identical before and after planning.
- **No deletes.** The planner never emits a `DeleteFile` capability.
- **No silent overwrite.** Target collisions are resolved by de-duplication and
  reported as conflicts.
- **Every action carries a policy decision.** Out-of-boundary moves come back
  `Deny` and stay in the plan for the reviewer to see.
- **Low-confidence items are flagged** (`confidence <= classifier::LOW_CONFIDENCE`)
  so the approval UI can route them to manual review.
- **Dated renaming is opt-in and idempotent.** Existing Zones keep original names
  unless `rename_dated` is enabled, and re-planning does not stack prefixes.
- **Deterministic.** The same tree and rules always produce the same plan.

## Status

Implemented: `scanner`, `classifier`, `naming`, `conflict`, `planner`, with unit
tests covering scan filtering, invoice/receipt/statement classification, dated
renaming, safe-naming + de-duplication, conflict detection, and end-to-end
planning (no-mutation, per-action decisions, delete-free, out-of-boundary deny,
low-confidence flagging, and loading Zone options/rules from storage). The
executor that applies an approved plan — with audit logging and undo journaling
— exists; see `docs/organizer-executor.md`.

The Organizer planner/executor is wired to Tauri through
`src-tauri/src/commands/organizer.rs` and registered in `src-tauri/src/lib.rs`.
The command bridge deliberately keeps `organizer_plan` read-only and makes
`organizer_execute` re-plan server-side before applying changes, so the preview
and execution path both use deterministic backend logic.
