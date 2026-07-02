# Command Registry

Operational rules for Tauri commands in Ghost.

Read this before adding, moving, renaming, or exposing commands.

## Canonical agent contract

`AGENTS.md` is the source of truth for AI agent behavior. This file applies that contract to the command surface.

Every meaningful command should support the product loop:

```text
Intent -> Plan -> Policy check -> User approval -> Execution -> Audit log -> Undo path
```

## Registry rule

`src-tauri/src/commands.rs` should stay a thin registry/re-export surface.

Do not place large command implementations directly in `commands.rs`.

Use the right module, then re-export/register from the registry layer.

## Modules

| Module | Purpose | Allowed shape |
|---|---|---|
| `commands/core.rs` | Stable local automation: permissions, recording, replay, inspection, workflow storage | explicit, tested, user-started |
| `commands/auth.rs` | Local password state and at-rest workflow protection | local-only, no network dependency |
| `commands/diagnostics.rs` | Config summaries, telemetry export, performance/debug data | read-first, redacted, user-initiated export |
| `commands/updates.rs` | Signed auto-update: read-only check + user-approved install | signature-verified, user-gated install |
| `commands/organizer.rs` | Ghost Organizer: Zone/rule management + plan/execute/undo for safe file organization | policy-gated, read-only plan, audited + undoable execution |
| `commands/experimental.rs` | AI, observer mode, cloud sync, analytics, visual checks, data sources, research features | gated, labeled, not default product UI |

## Required command metadata

When adding or changing a command, document these in code comments or nearby docs:

- command name;
- module;
- risk class;
- whether it reads user content;
- whether it mutates local state;
- whether it touches network/remote state;
- whether approval is required;
- whether audit logging is required;
- whether undo is possible;
- whether the command is stable or experimental.

## Risk classes

| Class | Meaning | Examples | Requirement |
|---|---|---|---|
| `safe-read` | Reads non-sensitive local metadata | list workflows, read settings | normal user action |
| `sensitive-read` | Reads user content or private context | file contents, selected text, window titles, screenshots | scoped permission and visible state |
| `local-mutate` | Changes local state | move file, rename file, delete workflow | plan, approval, audit, undo when possible |
| `external-mutate` | Sends data or changes remote state | upload, send message, submit form, sync | deny by default unless explicitly scoped |
| `os-control` | Controls input or other apps | click replay, typing, shell command | approved target scope and emergency stop |
| `experimental` | Not trusted product behavior yet | AI-generated workflows, observer suggestions | gated, labeled, isolated |

If a command fits more than one class, use the highest-risk class.

## Stable command requirements

A command can be part of the stable core only when it has:

1. clear user-facing behavior;
2. scoped inputs;
3. documented failure modes;
4. validation for invalid input;
5. interruption/cancel behavior where applicable;
6. audit behavior for meaningful operations;
7. undo support for reversible mutations where practical;
8. tests or a documented validation path.

## Experimental command rules

Experimental commands may exist for development, but they must:

- live in `commands/experimental.rs` or another clearly experimental module;
- be hidden from default product UI unless explicitly requested;
- avoid direct mutation unless routed through policy and approval;
- avoid cloud/network behavior unless clearly scoped;
- include limits and failure modes before promotion.

## Naming

Keep existing Tauri command names stable unless there is a migration plan.

When renaming a command:

1. update frontend calls;
2. update command registration;
3. update tests or fixtures;
4. update docs;
5. leave compatibility wrappers only when needed.

## Promotion path

An experimental command can move toward the stable core only after:

1. it has clear user-facing behavior;
2. it has failure modes documented;
3. it has tests for valid, invalid, denied, and interrupted flows where relevant;
4. it does not weaken local privacy;
5. it does not bypass approval for meaningful mutation;
6. it writes audit/undo data where required;
7. it is reflected in `docs/core-boundaries.md` and `AGENTS.md` if the product contract changes.

## Risk inventory

Every registered Tauri command (source of truth: `generate_handler!` in `src-tauri/src/lib.rs`) is inventoried below, grouped by module. **New command PRs must add a row here.** High- and critical-risk commands must require explicit approval, stay developer-only, or be absent from the default product UI. The legacy recording/replay surface is not yet wired to the policy engine (`src-tauri/src/policy/`); the **Ghost Organizer** commands (`commands/organizer.rs`) are the first surface that is — every proposed and executed action passes through `policy::evaluate`, and the executor writes an audit log and undo journal.

Legend — what the command touches: **Files** = local filesystem · **OS** = OS input synthesis/capture · **Scr** = screen contents / accessibility tree · **Net** = network · **Auth** = authentication or secrets · **Win** = app/window state. `✓` yes · `–` no · `~` conditional.

### `commands/core.rs` — stable core

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `start_recording` | stable | – | ✓ | – | – | – | ✓ | high | Captures keys/clicks; requires visible active state + granted permissions. Fails closed if accessibility/input-monitoring denied. |
| `stop_recording` | stable | – | ✓ | – | – | – | ✓ | low | Ends capture; no-op if not recording. |
| `replay_workflow` | stable | – | ✓ | – | – | – | ✓ | critical | Synthesizes real input. Must route through policy before broad use; wrong focused app/window can misfire. |
| `ghost_guard_audit` | stable | – | – | – | – | – | – | low | Pure deterministic risk audit of recorded events. |
| `get_replay_history` | stable | ✓ | – | – | – | – | – | low | Reads past replay runs (status, duration, failure reason, per-click resolution trace) from local execution history; `limit` caps rows. |
| `get_replay_progress` | stable | – | – | – | – | – | – | low | In-memory getter: current/total step of the running replay plus the last failed step index. Polled by the UI for per-step status. |
| `dry_run_workflow` | stable | – | – | – | – | – | – | low | Pure preview of what a replay would do (per-step action/target/coords). Executes nothing; typed text never included. |
| `cancel_replay` | stable | – | ✓ | – | – | – | – | low | Interrupts the replay loop. |
| `pause_replay` | stable | – | ✓ | – | – | – | – | low | Pauses the replay loop. |
| `resume_replay` | stable | – | ✓ | – | – | – | – | low | Resumes the replay loop. |
| `is_replay_paused` | stable | – | – | – | – | – | – | low | Getter. |
| `is_replay_running` | stable | – | – | – | – | – | – | low | Getter. |
| `set_playback_speed` | stable | – | – | – | – | – | – | low | Engine state; value clamped. |
| `get_playback_speed` | stable | – | – | – | – | – | – | low | Getter. |
| `inspect_element` | stable | – | – | ✓ | – | – | ✓ | high | Reads the UI element at coordinates via the accessibility API; returns app/window metadata. |
| `inspect_element_at_cursor` | stable | – | – | ✓ | – | – | ✓ | high | Same, at the current cursor position. |
| `save_workflow` | stable | ✓ | – | – | – | ~ | – | medium | Writes workflow JSON to the data dir; encrypted when local auth is configured. Name sanitized. |
| `load_workflow` | stable | ✓ | – | – | – | ~ | – | medium | Reads workflow JSON; decryption requires unlock. |
| `delete_workflow` | stable | ✓ | – | – | – | – | – | medium | Deletes a *workflow file* in app data (never a user file). Name sanitized. |
| `list_workflows` | stable | ✓ | – | – | – | – | – | low | Lists saved workflow files. |
| `get_recorded_events` | stable | – | – | – | – | – | – | low | Returns the in-memory event buffer. |
| `check_accessibility` | stable | – | – | – | – | – | ~ | low | Reports OS accessibility-permission state. |
| `request_accessibility` | stable | – | – | – | – | – | ~ | medium | Triggers the OS accessibility-permission prompt. |
| `check_input_monitoring` | stable | – | – | – | – | – | ~ | low | Reports OS input-monitoring permission state. |
| `request_input_monitoring` | stable | – | – | – | – | – | ~ | medium | Triggers the OS input-monitoring prompt. |

### `commands/auth.rs` — auth / at-rest protection (stable)

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `auth_status` | stable | – | – | – | – | ✓ | – | low | Reports configured/unlocked state. |
| `auth_setup` | stable | ✓ | – | – | – | ✓ | – | high | Creates the password, wraps the DEK, atomically writes `auth.json`. Losing that file makes encrypted workflows unrecoverable. |
| `auth_unlock` | stable | ✓ | – | – | – | ✓ | – | high | Derives the KEK from the password; a wrong password returns `false`, not an error. |
| `auth_lock` | stable | – | – | – | – | ✓ | – | low | Drops the in-memory data key. |

### `commands/diagnostics.rs` — diagnostics (stable)

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `get_config` | stable | ✓ | – | – | – | – | – | low | Reads the config file. |
| `update_config` | stable | ✓ | – | – | – | – | – | medium | Validates then writes config; rejects invalid values. |
| `get_telemetry_stats` | stable | – | – | – | – | – | – | low | Local in-memory counters. |
| `export_telemetry` | stable | ✓ | – | – | – | – | – | low | Exports anonymized telemetry; opt-in. |
| `get_performance_summary` | stable | – | – | – | – | – | – | low | In-memory performance metrics. |

### `commands/updates.rs` — signed auto-update (stable)

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `check_for_update` | stable | – | – | – | ✓ | – | – | medium | Read-only query of the update endpoint; downloads/changes nothing. Returns `None` when current. Failures (no endpoint / unconfigured key) are swallowed in the UI so launch is never blocked. |
| `install_update` | stable | ✓ | – | – | ✓ | – | ✓ | high | Downloads, **verifies the signature against the embedded public key**, replaces the app, and relaunches. **User-gated by design** — the UI calls this only after explicit "Update now". A failed verification installs nothing. |

### `commands/organizer.rs` — Ghost Organizer (stable)

The wedge product's trust pipeline, surfaced end to end. The plan step is
read-only and mutates nothing; execution re-checks policy per action, refuses to
overwrite, writes undo data before each mutation, and records an audit event for
every action. `organizer_execute` deliberately does **not** accept a plan from
the frontend — it re-plans server-side from the Zone id, so a stale or tampered
plan can never reach the filesystem.

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `organizer_list_zones` | stable | ✓ | – | – | – | – | – | low | Reads Zones from the local SQLite DB. |
| `organizer_list_folder_rules` | stable | ✓ | – | – | – | – | – | low | Reads a Zone's folder rules (the approved boundaries) from the DB. |
| `organizer_create_zone` | stable | ✓ | – | – | – | – | – | low | Inserts a Zone (DB only). Params: `name`, `description`, optional `renameDated` (default `false`). New Zones default to `Ask`; dated renaming only changes previewed destination names when explicitly enabled. |
| `organizer_add_folder_rule` | stable | ✓ | – | – | – | – | – | medium | Persists a user-approved boundary (DB only). Refuses rules granting delete. |
| `organizer_plan` | stable | ✓ | – | – | – | – | – | low | **Read-only.** Scans directory metadata, classifies, detects conflicts, policy-checks every action; mutates nothing. This is the preview the user approves. |
| `organizer_execute` | stable | ✓ | – | – | – | – | – | medium | **local-mutate.** Re-plans, re-checks policy per action, never overwrites, writes undo before each mutation, records an audit event, and persists the run. Moves/renames only inside an approved Zone; never deletes. |
| `organizer_list_executions` | stable | ✓ | – | – | – | – | – | low | Lists past executions for the history/undo view (DB only). |
| `organizer_undo` | stable | ✓ | – | – | – | – | – | medium | **local-mutate.** Replays a stored undo journal in reverse; never overwrites an occupied origin and never removes a non-empty folder. |

### `commands/experimental.rs` — experimental (gated / not default UI)

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `analyze_workflow` | experimental | – | – | – | – | – | – | medium | Local deterministic analysis. |
| `optimize_workflow` | experimental | – | – | – | – | – | – | medium | Suggests step cleanups; suggestion-only. |
| `suggest_workflow_name` | experimental | – | – | – | – | – | – | low | Heuristic name suggestion. |
| `save_workflow_with_metadata` | experimental | ✓ | – | – | – | ~ | – | medium | Writes workflow + metadata. |
| `load_workflow_with_metadata` | experimental | ✓ | – | – | – | ~ | – | medium | Reads workflow + metadata. |
| `generate_workflow_from_prompt` | experimental | ~ | – | – | ~ | – | – | critical | AI-drafted workflow. **Suggestion-only — must never execute directly.** Network if a remote provider is configured; prompt is validated. |
| `analyze_and_tag_workflow` | experimental | – | – | ✓ | – | – | ✓ | critical | Scans visible elements (accessibility traffic) to tag events. |
| `save_workflow_with_sidecar` | experimental | ✓ | – | – | – | ~ | – | medium | Writes workflow + reliability sidecar. |
| `replay_with_visual_check` | experimental | ✓ | ✓ | ✓ | – | – | ✓ | critical | Replays input and compares screenshots; captures the screen to disk. |
| `capture_baseline_screenshot` | experimental | ✓ | – | ✓ | – | – | – | high | Captures the full screen to disk. |
| `create_data_source` | experimental | ✓ | – | – | – | – | – | medium | Registers a data source; CSV reads are confined to the data dir. |
| `load_variables` | experimental | ✓ | – | – | – | – | – | medium | Reads variables from a data source; errors on missing column/row. |
| `replay_with_reliability` | experimental | – | ✓ | ✓ | – | – | ✓ | critical | Self-healing replay with element re-resolution. |
| `init_cloud_sync` | experimental | – | – | – | ✓ | ~ | – | critical | Cloud sync. **In-memory stub, hidden from MVP.** |
| `cloud_authenticate` | experimental | – | – | – | ✓ | ✓ | – | critical | Stub; would transmit a token. |
| `cloud_sync_workflows` | experimental | ✓ | – | – | ✓ | – | – | critical | Stub; would upload workflows. |
| `create_workspace` | experimental | – | – | – | ✓ | – | – | critical | Stub. |
| `get_audit_logs` | experimental | – | – | – | – | – | – | medium | Returns in-memory cloud audit logs (stub), newest first; `limit` = most recent N. |
| `get_execution_history` | experimental | ✓ | – | – | – | – | – | low | Execution analytics for one workflow. |
| `get_all_executions` | experimental | ✓ | – | – | – | – | – | low | Execution analytics across workflows. |
| `get_workflow_analytics` | experimental | ✓ | – | – | – | – | – | low | Aggregated analytics. |
| `start_observer` | experimental | – | – | ✓ | – | – | ✓ | critical | Proactive observer of the active app. **Hidden from MVP.** |
| `stop_observer` | experimental | – | – | – | – | – | – | low | Stops the observer. |
| `is_observer_active` | experimental | – | – | – | – | – | – | low | Getter. |
| `set_observer_interval` | experimental | – | – | – | – | – | – | low | Observer config. |
| `observe_events` | experimental | – | – | ✓ | – | – | ✓ | high | Reads active app/window events for one polling pass. |
| `get_proactive_suggestions` | experimental | – | – | – | – | – | – | medium | Learned-pattern suggestions; suggestion-only. |
| `get_learned_patterns` | experimental | – | – | – | – | – | – | low | Returns learned patterns. |
| `get_app_usage_stats` | experimental | – | – | – | – | – | ✓ | low | App/window usage stats. |
| `generate_geek_insights` | experimental | – | – | – | – | – | – | medium | Diagnostic ("geek mode") insights. |

## Agent checklist

Before finishing command work, verify:

- the command is in the right module;
- the risk class is clear;
- risky operations go through policy;
- user approval is required where needed;
- audit and undo behavior are addressed;
- experimental work is gated or labeled;
- docs were updated with the behavior change;
- checks were run or the validation gap is reported.
