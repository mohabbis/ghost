# Command Registry

Ghost exposes Tauri commands through `src-tauri/src/commands.rs`, but command implementations are split by product boundary.

## Modules

| Module | Purpose |
|---|---|
| `commands/core.rs` | Stable local automation surface: recording, replay, inspection, workflow storage, and permissions. |
| `commands/auth.rs` | Local password state and at-rest workflow protection controls. |
| `commands/diagnostics.rs` | Config, telemetry export, and performance summaries. |
| `commands/experimental.rs` | AI, observer mode, cloud sync, analytics, visual checks, data sources, and reliability experiments. |

## Policy

New commands should not be added directly to `commands.rs`. Put the implementation in the right module, then re-export it through the registry.

Stable core commands should remain boring, explicit, and testable. Experimental commands can move quickly, but they should stay visibly separated until they have documented reliability and UX limits. Yes, this is bureaucracy, but it is the useful kind, not the kind that makes you upload the same PDF six times.

## Naming

Keep existing Tauri command names stable unless there is a migration plan. Frontend calls depend on these names, and breaking them silently is how apps become haunted.

## Promotion path

An experimental command can move toward the stable core only after:

1. It has clear user-facing behavior.
2. It has failure modes documented.
3. It has tests for valid, invalid, and interrupted flows.
4. It does not weaken local privacy or workflow safety.
5. It is reflected in `docs/core-boundaries.md`.

## Risk inventory

Every registered Tauri command (source of truth: `generate_handler!` in
`src-tauri/src/lib.rs`) is inventoried below, grouped by module. **New command
PRs must add a row here.** High- and critical-risk commands must require
explicit approval, stay developer-only, or be absent from the default product
UI; none are wired to the policy engine (`src-tauri/src/policy/`) yet — that
happens when Ghost Organizer lands.

Legend — what the command touches: **Files** = local filesystem · **OS** = OS
input synthesis/capture · **Scr** = screen contents / accessibility tree ·
**Net** = network · **Auth** = authentication or secrets · **Win** = app/window
state. `✓` yes · `–` no · `~` conditional.

### `commands/core.rs` — stable core

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `start_recording` | stable | – | ✓ | – | – | – | ✓ | high | Captures keys/clicks; requires visible active state + granted permissions. Fails closed if accessibility/input-monitoring denied. |
| `stop_recording` | stable | – | ✓ | – | – | – | ✓ | low | Ends capture; no-op if not recording. |
| `replay_workflow` | stable | – | ✓ | – | – | – | ✓ | critical | Synthesizes real input. Must route through policy before broad use; wrong focused app/window can misfire. |
| `ghost_guard_audit` | stable | – | – | – | – | – | – | low | Pure deterministic risk audit of recorded events. |
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
| `get_audit_logs` | experimental | – | – | – | – | – | – | medium | Returns in-memory cloud audit logs (stub). |
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
