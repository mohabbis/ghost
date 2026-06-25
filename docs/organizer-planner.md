# Organizer Planner

The Organizer planner turns an approved Zone into a **reviewable plan that
mutates nothing**. It is the `Plan` (and `Policy`) step of the trust pipeline:

```text
Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
         ^^^^^^^^^^^^^^^^^
         this module; the executor (Execution/Audit/Undo) is a later phase.
```

It lives in `src-tauri/src/organizer/` and performs **only read-only filesystem
access** (directory listing + `stat`, never file contents). It reuses the
`policy` engine for every decision and loads boundaries from `storage`.

## Flow

`planner::plan_zone(conn, zone_id)` loads the Zone's `FolderRule`s via
`storage::zones::list_folder_rules` and calls `plan_with_rules(zone_id, &rules)`,
which is also exposed directly so the planner is testable without a database:

1. **Scan** every readable source folder (`scanner::scan`), bounded to
   `scanner::MAX_DEPTH`, de-duplicating files reachable from overlapping roots
   and sorting by path for deterministic output.
2. **Filter** system/ignored entries (`scanner::is_ignored`): dotfiles,
   `Thumbs.db`/`desktop.ini`/recycle-bin metadata, and temp/partial-download
   suffixes (`~`, `.tmp`, `.part`, `.crdownload`, …).
3. **Classify** each file deterministically (`classifier::classify`): extension
   first (mapped to a `Category`), enriched with filename keyword signals
   (`invoice`, `receipt`, `screenshot`, …). Unknown/absent extensions become
   `Category::Other` with low confidence. No AI, no IO. Each result carries a
   `confidence` in `0.0..=1.0` and a human-readable `reason`.
4. **Propose a target**: `<destination root>/<Category>/<safe name>`. The
   destination root is the first folder rule (ordered by path) granting both
   *create* and *move*. The safe name comes from `naming::safe_file_name`
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
- **Deterministic.** The same tree and rules always produce the same plan.

## Status

Implemented: `scanner`, `classifier`, `naming`, `conflict`, `planner`, with unit
tests covering scan filtering, classification, safe-naming + de-duplication,
conflict detection, and end-to-end planning (no-mutation, per-action decisions,
delete-free, out-of-boundary deny, low-confidence flagging, and loading rules
from storage). The executor that applies an approved plan — with audit logging
and undo journaling — now exists; see `docs/organizer-executor.md`. Both remain
**backend-only and not yet wired** to any Tauri command. Each command exposed at
that point gets a row in `docs/command-registry.md`.
