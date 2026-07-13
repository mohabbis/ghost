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
| `commands/account.rs` | Microsoft/Google OAuth sign-in (identity only, no data movement) | network-scoped to the provider's own authorize/token/userinfo endpoints; no other outbound calls |
| `commands/intelligence.rs` | Internal intelligence providers (suggestion-only Organizer planning) — **experimental**, gated behind `--features experimental` | explicit API keys (encrypted at rest), redacted metadata outbound, no execution |
| `commands/diagnostics.rs` | Config summaries, telemetry export, performance/debug data | read-first, redacted, user-initiated export |
| `commands/updates.rs` | Signed auto-update: read-only check + user-approved install | signature-verified, user-gated install |
| `commands/organizer.rs` | Ghost Organizer: Zone/rule management + plan/execute/undo for safe file organization | policy-gated, read-only plan, audited + undoable execution |
| `commands/integrations.rs` | Power BI + Fabric audit export: grant flow, preview, push (Power BI only) — **experimental**, gated behind `--features experimental` | explicit revocable grant separate from identity, preview-before-push, PII-masked, re-derives export payload server-side rather than trusting the frontend |
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

Every registered Tauri command (source of truth: `generate_handler!` in `src-tauri/src/lib.rs`) is inventoried below, grouped by module. **New command PRs must add a row here.** High- and critical-risk commands must require explicit approval, stay developer-only, or be absent from the default product UI. The **Ghost Organizer** commands (`commands/organizer.rs`) route every proposed and executed filesystem action through `policy::evaluate`. Recorded routines: `routine_policy_plan` previews per-step `os-*` decisions; `approve_routine_replay` stores a one-shot server-side approval; `replay_workflow` re-derives the plan, refuses `Deny`, and consumes that approval before synthesizing input. Undo/vault for routines remains follow-up work.

Legend — what the command touches: **Files** = local filesystem · **OS** = OS input synthesis/capture · **Scr** = screen contents / accessibility tree · **Net** = network · **Auth** = authentication or secrets · **Win** = app/window state. `✓` yes · `–` no · `~` conditional.

### `commands/core.rs` — stable core

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `start_recording` | stable | – | ✓ | – | – | – | ✓ | high | Captures keys/clicks; requires visible active state + granted permissions. Fails closed if accessibility/input-monitoring denied. |
| `stop_recording` | stable | – | ✓ | – | – | – | ✓ | low | Ends capture; no-op if not recording. |
| `replay_workflow` | stable | – | ✓ | – | – | – | ✓ | critical | Synthesizes real input. Re-derives policy plan; refuses `Deny`; requires a matching one-shot `approve_routine_replay` token. Wrong focused app/window can still misfire. |
| `approve_routine_replay` | stable | – | – | – | – | – | – | high | Records a TTL-bound, fingerprint-keyed approval after re-deriving the policy plan. Does not execute. Consumed by `replay_workflow`. |
| `ghost_guard_audit` | stable | – | – | – | – | – | – | low | Pure deterministic risk audit of recorded events. |
| `ghost_guard_audit_compressed` | stable | – | – | – | – | – | – | low | Pure deterministic risk audit of the compressed **semantic timeline**: compresses events server-side (no LLM/network) and audits the resulting steps, so findings map to review-timeline step indices, not raw events. |
| `routine_policy_plan` | stable | – | – | – | – | – | – | low | Preview-only: compresses events server-side and evaluates each semantic step through `policy::evaluate` as `os_*` capabilities. Never executes. Secure-field typing and unknown steps are denied; other OS steps require confirmation until app Zones exist. |
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
| `run_ocr_on_image` | stable | – | – | ✓ | – | – | – | high | Runs local OCR (macOS Vision / Windows OCR) on user-supplied image bytes; returns text blocks with normalized bounds. No network. |
| `parse_id_document` | stable | – | – | – | – | – | – | low | Deterministic ID-document parsing (`core/id_scan.rs`) over already-OCR'd text; returns structured fields + derived signals (age, expiry state, review flags). Pure text, no image/IO/network. |
| `save_workflow` | stable | ✓ | – | – | – | ~ | – | medium | Writes workflow JSON to the data dir; encrypted when local auth is configured. Name sanitized. |
| `load_workflow` | stable | ✓ | – | – | – | ~ | – | medium | Reads workflow JSON; decryption requires unlock. |
| `delete_workflow` | stable | ✓ | – | – | – | – | – | medium | Deletes a *workflow file* in app data (never a user file). Name sanitized. |
| `list_workflows` | stable | ✓ | – | – | – | – | – | low | Lists saved workflow files. |
| `get_recorded_events` | stable | – | – | – | – | – | – | low | Returns the in-memory event buffer. |
| `check_accessibility` | stable | – | – | – | – | – | ~ | low | Reports OS accessibility-permission state. |
| `request_accessibility` | stable | – | – | – | – | – | ~ | medium | Triggers the OS accessibility-permission prompt. |
| `check_input_monitoring` | stable | – | – | – | – | – | ~ | low | Reports OS input-monitoring permission state. |
| `request_input_monitoring` | stable | – | – | – | – | – | ~ | medium | Triggers the OS input-monitoring prompt. |
| `restart_app` | stable | – | – | – | – | – | ✓ | medium | Relaunches the app so macOS re-evaluates a fresh permission grant. App/window state only; no data touched. |

### `commands/auth.rs` — auth / at-rest protection (stable)

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `auth_status` | stable | – | – | – | – | ✓ | – | low | Reports configured/unlocked state. |
| `auth_setup` | stable | ✓ | – | – | – | ✓ | – | high | Creates the password, wraps the DEK, atomically writes `auth.json`. Losing that file makes encrypted workflows unrecoverable. |
| `auth_unlock` | stable | ✓ | – | – | – | ✓ | – | high | Derives the KEK from the password; a wrong password returns `false`, not an error. |
| `auth_lock` | stable | – | – | – | – | ✓ | – | low | Drops the in-memory data key. |

### `commands/account.rs` — account sign-in (stable, network-scoped)

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `account_status` | stable | ✓ | – | – | – | ✓ | – | low | Reads the local linked-account record (via the vault's protect/reveal envelope). Reports `google_sign_in_available` / `microsoft_sign_in_available` from configured or bundled OAuth client IDs. Reports "not signed in" if locked or unlinked — never errors. |
| `account_sign_in` | stable | ✓ | – | – | ✓ | ✓ | – | high | Opens the system browser (OAuth 2.0 + PKCE) and a loopback listener, then calls the provider's token + userinfo endpoints. Requires `integrations.microsoft_client_id`/`google_client_id` (or `GHOST_MS_CLIENT_ID`/`GHOST_GOOGLE_CLIENT_ID`) to be configured, else fails with an actionable error. Errors on state mismatch, cancellation, or a non-2xx provider response. Identity only — establishes no data-access grant to any other product surface. |
| `account_sign_out` | stable | ✓ | – | – | – | ✓ | – | low | Deletes the local linked-account record. Does not revoke provider-side consent — the user should also remove Ghost from their account's connected-apps list if they want that. |

### `commands/intelligence.rs` — internal intelligence providers (experimental, suggestion-only)

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `intelligence_provider_status` | experimental | ✓ | – | – | ~ | ✓ | – | low | Reports configured state and health per provider; never returns API keys. |
| `intelligence_set_api_key` | experimental | ✓ | – | – | – | ✓ | – | medium | Encrypts and stores provider API key locally (`intelligence-secrets.json`). |
| `intelligence_clear_api_key` | experimental | ✓ | – | – | – | ✓ | – | low | Removes one provider key from local storage. |
| `intelligence_test_provider` | experimental | – | – | – | ✓ | ✓ | – | medium | Network health check (OpenAI models list; Anthropic configured check). |
| `intelligence_propose_plan` | experimental | ~ | – | – | ✓ | ✓ | – | high | Sends redacted file metadata to the configured provider; returns a `PlanningSuggestion` only — **never executes**. Blocks confidential/secret payloads when local-only routing is enabled. |
| `intelligence_discover_local` | experimental | ✓ | – | – | ~ | ✓ | – | low | Probes localhost Ollama/LM Studio ports only; returns discovered models. |
| `organizer_intelligence_suggest` | experimental | ~ | – | – | ✓/local | ✓ | – | high | Scans Zone file metadata, calls the configured intelligence provider, returns suggestion-only output validated by `suggestion_is_safe`. Never executes. |

### `commands/integrations.rs` — Power BI audit export (experimental, network-write)

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `power_bi_grant_status` | experimental | ✓ | – | – | – | ✓ | – | low | Reads local grant metadata only; never errors. |
| `power_bi_request_grant` | experimental | ✓ | – | – | ✓ | ✓ | – | high | Opens the system browser (incremental-consent OAuth) for the Power BI API scope. Requires an existing signed-in Microsoft identity — fails with `AuthenticationRequired` otherwise. Identity link is unaffected; this is an additional, separate grant. |
| `power_bi_revoke_grant` | experimental | ✓ | – | – | – | ✓ | – | low | Local unlink only (`revoked_at`); does not revoke provider-side consent. |
| `power_bi_export_preview` | experimental | ✓ | – | – | – | – | – | low | Pure, read-only: assembles the exact `GhostRuns`/`GhostActions`/`GhostPolicyEvents` payload a push would send from local Organizer execution history, PII-masked. No network. `since_days` bounds the window (omit for all history). |
| `power_bi_push_audit_export` | experimental | ✓ | – | – | ✓ | ✓ | – | high | The only command that performs the network write. Requires an active Power BI grant; re-derives the export payload server-side (never trusts a frontend-supplied snapshot — same principle `organizer_execute` applies to Organizer plans) and pushes to a dataset named `GhostOperations` in the signed-in user's own Power BI workspace, creating it on first use. |
| `fabric_grant_status` | experimental | ✓ | – | – | – | ✓ | – | low | Reads local Fabric grant metadata only. |
| `fabric_request_grant` | experimental | ✓ | – | – | ✓ | ✓ | – | high | Incremental OAuth consent for `api.fabric.microsoft.com/.default`. Requires Microsoft sign-in first. |
| `fabric_revoke_grant` | experimental | ✓ | – | – | – | ✓ | – | low | Local Fabric grant revoke only. |
| `fabric_list_workspaces` | experimental | ✓ | – | – | ✓ | ✓ | – | medium | Lists Fabric workspaces; requires active Fabric grant. |
| `fabric_export_preview` | experimental | ✓ | – | – | – | – | – | low | Read-only audit export preview (same row shapes as Power BI). No network. |
| `fabric_list_lakehouses` | experimental | ✓ | – | – | ✓ | ✓ | – | medium | Lists lakehouse items in a workspace; requires Fabric grant. |
| `fabric_push_audit_export` | experimental | ✓ | – | – | ✓ | ✓ | – | high | Uploads JSON export files to a lakehouse `Files/ghost-export/` path via OneLake. Re-derives payload server-side; requires preview in UI first. |
| `fabric_list_inbound_intents` | experimental | ✓ | – | – | – | – | – | low | Lists pending inbound Fabric intents (no auto-execute). |
| `fabric_dismiss_inbound_intent` | experimental | ✓ | – | – | – | – | – | low | Dismisses an inbound intent without acting on it. |
| `fabric_record_inbound_intent` | experimental | ✓ | – | – | – | – | – | low | Records an inbound intent for Organizer review (webhook simulation). |
| `fabric_webhook_status` | experimental | ✓ | – | – | – | – | – | low | Reports whether a webhook secret is configured. |
| `fabric_set_webhook_secret` | experimental | ✓ | – | – | – | – | – | medium | Generates/rotates the local `X-Ghost-Webhook-Secret` for `POST /fabric/webhook`. |
| `google_grant_status` | experimental | ✓ | – | – | – | – | – | low | Local Google Cloud grant metadata. |
| `google_request_grant` | experimental | ✓ | – | – | ✓ | – | – | medium | OAuth consent for GCS export scope. |
| `google_revoke_grant` | experimental | ✓ | – | – | – | – | – | low | Revokes local Google Cloud grant. |
| `google_list_buckets` | experimental | ✓ | – | – | ✓ | – | – | low | Lists buckets in a GCP project (requires `project_id`). |
| `google_export_preview` | experimental | ✓ | – | – | – | – | – | low | Read-only audit export preview for GCS push. |
| `google_push_audit_export` | experimental | ✓ | – | – | ✓ | ✓ | – | high | Pushes JSON export objects to a user-chosen GCS bucket; enforces bound bucket when scoped. |
| `google_bind_export_bucket` | experimental | ✓ | – | – | – | – | – | low | Narrows Google Cloud grant to one GCS bucket (`ResourceScope::Destination`). |

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
| `organizer_list_zones` | stable | ✓ | – | – | – | – | – | low | Reads Zones from the local DB. |
| `organizer_list_folder_rules` | stable | ✓ | – | – | – | – | – | low | Reads a Zone's folder rules (the approved boundaries) from the DB. |
| `organizer_default_paths` | stable | ✓ | – | – | – | – | – | low | **safe-read.** Returns real Downloads/home/Documents paths for first-run Organizer UI. |
| `organizer_create_zone` | stable | ✓ | – | – | – | – | – | low | Inserts a Zone (DB only). Params: `name`, `description`, optional `renameDated` (default `false`). New Zones default to `Ask`; dated renaming only changes previewed destination names when explicitly enabled. |
| `organizer_add_folder_rule` | stable | ✓ | – | – | – | – | – | medium | Persists a user-approved boundary (DB only). Refuses rules granting delete. Accepts an optional `trust` (`automate`/`ask_first`/`never`); defaults to `ask_first` when omitted so old frontends behave unchanged. |
| `organizer_set_rule_trust` | stable | ✓ | – | – | – | – | – | medium | **local-mutate (DB only).** Updates an existing rule's trust level by its path within the Zone. Errors if no such rule exists. Trust is *recorded* here but *enforced* server-side by the policy engine + executor — the frontend can't bypass it. |
| `organizer_plan` | stable | ✓ | – | – | – | – | – | low | **Read-only.** Scans directory metadata, classifies, detects conflicts, policy-checks every action (recording which rule fired); mutates nothing. This is the preview the user approves. |
| `organizer_execute` | stable | ✓ | – | – | – | – | – | medium | **local-mutate.** Re-plans, re-checks policy per action, never overwrites, writes undo before each mutation, records an audit event (with the rule that fired and automated-vs-approved provenance), and persists the run **sealed into a tamper-evident hash chain**. Write-ahead durable: inserts an unsealed, unfinished row before the first mutation and durably updates it after every action (`storage::executions::begin_execution`/`update_execution_progress`), rather than holding the whole run in memory and writing once at the end — a crash mid-run leaves a recoverable record instead of losing the undo journal for whatever had already applied (see `docs/organizer-executor.md` "Crash recovery"). If the user set an audit retention policy, prunes older runs afterward (best-effort; a prune error never fails the run). Moves/renames only inside an approved Zone; never deletes. |
| `organizer_list_executions` | stable | ✓ | – | – | – | – | – | low | Lists past executions for the history/undo view (DB only); each row carries `sealed` and `finished` flags. |
| `organizer_check_unfinished_run` | stable | ✓ | – | – | – | – | – | low | **safe-read.** Returns the newest execution that began but never finished (`finished = 0`), or `null` if the last run ended cleanly. The frontend calls this on Organizer view load to surface a recover-or-dismiss prompt. |
| `organizer_dismiss_unfinished_run` | stable | ✓ | – | – | – | – | – | low | **local-mutate (DB only).** Marks an interrupted run finished without undoing it or touching the filesystem — the user has reviewed what it applied and chose to leave those changes in place. |
| `organizer_undo` | stable | ✓ | – | – | – | – | – | medium | **local-mutate.** Replays a stored undo journal in reverse; never overwrites an occupied origin and never removes a non-empty folder. Works on unfinished (crash-interrupted) runs the same as finished ones — a successful undo also marks the run resolved. |
| `organizer_export_audit` | stable | ✓ | – | – | – | – | – | low | **safe-read.** Returns a past run's audit log as `json` or `csv` text (each row: action, outcome, the rule that fired, automated/user-approved provenance), with the run's tamper-evidence seal (`hash`/`prev_hash`) as export metadata. Writes nothing itself — the caller saves the returned text. |
| `organizer_time_to_value` | stable | ✓ | – | – | – | – | – | low | **safe-read.** Returns local first-touch milestone timestamps (first zone/plan/run/undo) for the diagnostics view. Local-only: timestamps, no paths or content, no network. |
| `organizer_verify_audit_chain` | stable | ✓ | – | – | – | – | – | low | **safe-read.** Recomputes the execution hash chain and reports whether every sealed run still matches its seal and links to the previous run (`intact`, `sealed_count`, `unsealed_count`, `first_break`). Offline tamper-evidence check; no network. |
| `organizer_issue_mcp_approval_token` | stable | ✓ | – | – | – | ✓ | – | medium | **safe-read.** Issues a signed, short-lived, single-use MCP token bound to the current server-side plan hash. Requires vault unlock; user must have reviewed the plan in Organizer. |

### `commands/mcp.rs` — MCP pairing (stable)

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `mcp_pairing_status` | stable | ✓ | – | – | – | – | – | low | Reports whether MCP stdio clients must supply a pairing code on `initialize`. |
| `mcp_enable_pairing` | stable | ✓ | – | – | – | – | – | low | Generates and persists a pairing code; shown once to the user. |
| `mcp_disable_pairing` | stable | ✓ | – | – | – | – | – | low | Clears pairing requirement. |
| `mcp_list_pending_approvals` | stable | ✓ | – | – | – | – | – | low | Lists pending MCP approval requests waiting for desktop review (polled by the UI). |
| `mcp_http_server_status` | experimental | ✓ | – | – | – | – | – | low | Reports MCP/Fabric HTTP listener state (bind, port, LAN exposure). |
| `mcp_start_http_server` | experimental | ✓ | – | – | – | – | – | high | Starts combined `POST /mcp` + `POST /fabric/webhook` listener; LAN requires bearer token; optional in-process TLS PEM paths. |
| `mcp_stop_http_server` | experimental | ✓ | – | – | – | – | – | low | Stops the HTTP listener started by `mcp_start_http_server`. |
| `mcp_relay_status` | experimental | ✓ | – | – | – | – | – | low | Reports outbound cloud relay connection state. |
| `mcp_start_relay` | experimental | ✓ | – | – | – | – | ✓ | high | Outbound HTTPS poll to user-hosted MCP relay (`docs/mcp-relay.md`). |
| `mcp_stop_relay` | experimental | ✓ | – | – | – | – | – | low | Stops outbound relay client. |

### `commands/filing.rs` — audience-aware filing preview (stable)

Read-only, name-only planning that proposes where recurring files belong, per
audience profile (`Finance`, `Student`; see `docs/filing-profiles.md` and
`docs/audiences.md`). These commands touch **nothing** — no filesystem, no
network, no OS input, no secrets. They reason purely over the file *names*
passed in, so they are safe to run on a paste-in list before any folder access
is granted. The actual move/rename still flows through the Organizer's audited,
undoable executor.

| Command | Stability | Files | OS | Scr | Net | Auth | Win | Risk | Failure modes / notes |
|---|---|:--:|:--:|:--:|:--:|:--:|:--:|---|---|
| `preview_file_filing` | stable | – | – | – | – | – | – | low | **safe-read.** Classifies each provided file name by the chosen `audience` profile and proposes a destination directory (engineering artifact/report type/coursework type + run period/reporting period/term), plus counts and per-item review flags. Deterministic; unrecognized files are surfaced for review, never guessed or mutated. `root` optional (audience default when empty). |
| `estimate_filing_savings` | stable | – | – | – | – | – | – | low | **safe-read.** Pure arithmetic: turns filing volume/cadence/time inputs into annual hours (and, with an hourly rate, cost) saved by an assisted workflow. Inputs are clamped defensively; the estimate echoes back every default applied. |

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
