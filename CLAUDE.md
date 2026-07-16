# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`AGENTS.md` is the canonical agent contract. Read and follow it first. This file adds Claude-specific execution guidance and should not conflict with `AGENTS.md`.

## Operating mode

Work like a cautious product engineer, not a demo agent chasing novelty.

Default behavior:

1. inspect the existing file or module before editing;
2. preserve the trust model;
3. keep changes scoped;
4. avoid broad rewrites unless explicitly requested;
5. update docs when behavior changes;
6. run relevant checks when possible;
7. report what changed and what was not validated.

## Product identity

Ghost is a local-first desktop automation product for macOS and Windows.

Canonical positioning:

> Ghost turns repeated computer work into safe, reusable, permission-bounded routines.

Do not frame Ghost as:

- a generic autonomous AI agent;
- a chatbot;
- an RPA clone;
- a macro recorder only;
- an app that silently takes over the user's computer.

The product value is trustworthy execution:

```text
Record -> Inspect -> Approve -> Replay -> Audit -> Undo
```

Ghost supports account sign-in (Microsoft/Google) and is meant to eventually connect with the tech stacks users already run — Microsoft Fabric/Power BI, Google Cloud, and AI-assistant connectors (Claude, Cursor, Codex, ChatGPT). Stack connectivity is a competitive advantage; it does not replace the trust pipeline above. See `docs/integrations-roadmap.md`.

## Current wedge

Prioritize Ghost Organizer before broad automation.

Ghost Organizer flow:

```text
Select folder -> Scan -> Propose plan -> Review -> Approve -> Move/Rename -> Audit -> Undo
```

Required behavior:

- preview every filesystem mutation;
- deny silent delete and silent overwrite;
- detect conflicts;
- require approval before mutation;
- write audit events;
- write undo data before reversible operations.

## Engineering rules

Every meaningful operation should pass through:

```text
Intent -> Plan -> Policy check -> User approval -> Execution -> Audit log -> Undo path
```

For recorded routines, the review path should move toward:

```text
Raw Input Capture -> Deterministic Compression -> Semantic Timeline -> Guard -> Policy -> Approval -> Execute -> Vault -> Undo
```

Rules:

1. AI may propose; deterministic code executes only approved plans.
2. Ghost denies risky actions by default.
3. High-risk commands require explicit approval or must remain unavailable in default UI.
4. New Tauri commands need a module and risk class.
5. Experimental features stay gated or labeled.
6. Coordinates are fallback automation identity, not the primary model.
7. Replay must be interruptible.
8. Reversible mutations must write undo data before execution.
9. Sensitive reads must be scoped and visible to the user.
10. Marketing/docs must not promise capabilities the app cannot support.

## Privacy boundaries

Default stance:

- no camera;
- no microphone;
- no hidden screen capture;
- no background email monitoring;
- no background browser/tab reading;
- no raw secret capture;
- no cloud-first storage for workflow/organizer data — it stays local and encrypted at rest, regardless of which account or stack integrations are enabled;
- keyboard/pointer capture only during explicit recording or approved replay.

Account sign-in (Microsoft/Google OAuth) and any stack integration (Fabric/Power BI, Google Cloud, AI-assistant connectors) are opt-in, scoped, and disclosed — see `docs/integrations-roadmap.md`. None of them weaken the privacy defaults above.

For Organizer, use explicit folder selection and local filesystem operations.

For Routines later, input recording requires visible active state, emergency stop, permission checks, sensitive-input suppression, and a deterministic review layer before execution.

## Target product layers

Build in this order:

1. **Ghost Organizer**
   - safe file/folder cleanup;
   - classification, naming, moving, conflict detection, preview, audit, undo.
2. **Ghost Routines**
   - explicit recorded routines across apps and websites;
   - deterministic event compression into reviewable semantic steps;
   - semantic replay with coordinates as fallback.
3. **Ghost Intelligence**
   - suggestion-only planning, classification, explanation, and routine detection.

Organizer first. Routines second. Intelligence last.

## Architecture direction

Keep Rust/Tauri. Do not rewrite the whole product before proving the wedge.

**Version / release state (read first):** the repo is on **Ghost 2.0** (`src-tauri/Cargo.toml` + `tauri.conf.json` at `2.0.2`). Ghost 2.0 converges Organizer, Routines, and MCP onto one **Action Plan** runtime (`Capture → Review → Approve → Execute → Verify → Recover`; see `docs/GHOST_2_DEMO.md`, `docs/native-macos-preview.md`). Note the **release gap**: the newest *published* GitHub release is still **v1.2.9** — the v2.0.x code is merged to `master` but not cut as a public installer. Some state docs (`docs/PROJECT_STATE.md`, `docs/full-repo-audit-2026-07-13.md`) predate 2.0 and describe v1.2.9; treat them as historical for anything below.

Current structure:

```text
src/                    # Tauri desktop frontend (ES-module JS/HTML/CSS, bundled by Vite; main.js holds most UI logic; compression-review.js/.css is the split-out event-review timeline; src/public/ holds pass-through static assets)
apps/macos/             # Ghost 2.0 native macOS app (SwiftUI): App/, Views/, Features/, Services/, RustBridge/, AppKitBridge/ — UI only; all trust decisions stay in the Rust core over a JSON stdin/stdout bridge (docs/native-macos-preview.md)
native/macos/           # GhostAXHelper.swift — read-only macOS Accessibility helper (list_matches op)
public/                 # marketing/download site (static vanilla JS with in-browser demos; ships Ghost.dmg / Ghost_Setup.exe under downloads/; auto-deployed to Vercel by deploy-website.yml)
src-tauri/              # Rust backend
docs/                   # planning and technical docs
.github/workflows/      # CI (rust.yml), release (release.yml), site deploy (deploy-website.yml)
```

Current backend layout:

```text
src-tauri/src/
  lib.rs                 # app wiring + generate_handler! registry (conflict hotspot — add entries in place)
  main.rs                # binary entrypoint
  bin/diagnose_perms.rs  # macOS-only dev binary: reports Accessibility/Input Monitoring grant state via IOKit
  engine.rs              # recording/replay engine state
  auth.rs, accounts.rs, config.rs, error.rs, performance.rs, telemetry.rs
                         # accounts.rs: thin compatibility shim over identity/ (see below), independent of the vault password in auth.rs
  commands.rs            # thin registry/re-export over commands/
  commands/
    core.rs              # stable automation, recording, replay (incl. get_replay_history + replay WAL/undo: replay_check_unfinished_run/_dismiss_unfinished_run/replay_undo), workflow storage, permissions, Ghost Guard audit, OCR, ID parsing
    auth.rs              # local password state and protection
    account.rs           # account_status/_sign_in/_sign_out: Microsoft/Google OAuth + PKCE sign-in (identity only, see docs/integrations-roadmap.md)
    compression.rs       # compress_workflow (event compression -> review timeline)
    diagnostics.rs       # config, telemetry export, performance, is_experimental_enabled
    organizer.rs         # organizer_plan/_execute/_list_executions/_undo, audit export, signed-report verify, policy pack import/export, audit-chain verify, time-to-value
    runtime_cmds.rs      # Ghost 2.0 Action Plan runtime commands: action_plan_from_zone/_from_events/_demo, execute_action_plan, execute_routine_action_plan, get_execution_receipt, undo_action_plan_execution
    mcp.rs               # STABLE MCP surface: mcp_pairing_status/_enable_pairing/_disable_pairing/_list_pending_approvals + organizer_issue_mcp_approval_token (HTTP server/relay commands are experimental-gated)
    filing.rs            # preview_file_filing / estimate_filing_savings (read-only, audience-aware filing preview: Finance/Student/Engineering)
    updates.rs           # updater surface
    intelligence.rs       # experimental (feature-gated): internal suggestion-only OpenAI/Anthropic planning providers
    integrations.rs       # experimental (feature-gated): Power BI audit-export grant flow/preview/push (docs/power-bi-integration.md)
    experimental.rs      # AI, observer, cloud sync, analytics, visual checks (feature-gated)
  identity/               # Layer A: account identity + integration grants, separate from the vault password
    types.rs, store.rs, errors.rs
    oauth/                # pkce.rs, provider.rs, callback.rs, flow.rs (run_sign_in_flow + run_grant_flow, sharing one authorize_and_exchange core)
  integrations/           # Layer B: business-system connectors, gated behind their own IntegrationGrant
    microsoft/            # mod.rs (grant checks + Power BI grant request), scopes.rs, fabric/ (stub), power_bi/ (real HTTP client + export.rs payload builder)
  intelligence/           # Layer C: internal suggestion-only AI providers (OpenAI/Anthropic), feature-gated
  mcp/                    # Ghost's MCP surface: pairing.rs, approval.rs, pending.rs, token_store.rs, plan_hash.rs, tools.rs, handlers.rs, execute.rs (stable) + http.rs, server.rs, relay.rs, tls.rs (HTTP transport, experimental-gated). docs/mcp-integration.md, docs/mcp-relay.md
  action_plan/            # Ghost 2.0: compile.rs/demo.rs/types.rs — compiles a Zone or recorded events into one reviewable, policy-evaluated Action Plan (the unified Organizer+Routines intent model)
  runtime/                # Ghost 2.0 Action Plan runtime: execute.rs/fs.rs/semantic.rs/ui.rs/verify.rs/receipt.rs/persist.rs — approve → execute → verify → receipt → recover, over both file (Organizer) and semantic-UI (Routines) steps
  core/
    compress.rs          # deterministic text compression for LLM-bound content
    compression/         # deterministic event compression for workflow review
    execution.rs         # ExecutionRecord tracking for replay history
    replay_support.rs    # shared pause/cancel/pacing replay plumbing + resolution chain/matcher
    template_match.rs    # stable: pure-Rust pixel template matching (no OpenCV/system lib), the resolution chain's last-resort strategy
    dry_run.rs           # pure per-step replay preview (typed text excluded)
    id_scan.rs           # deterministic identity-document field parsing (stable; text in, no image/AI)
    ocr.rs               # stable: local OCR (macOS Vision / Windows OCR) over user-supplied image bytes, no network
    oauth.rs             # thin compatibility re-export of identity/oauth/ for older imports — new code should import identity:: directly
    guard.rs             # Ghost Guard: keyboard-suppression heuristics + deterministic pre-replay/save workflow audit (docs/GHOST_GUARD.md)
    atlas.rs             # Ghost Atlas: pure, deterministic local semantic-memory engine (lexical embeddings — NOT neural; no model, no network, no I/O). Persistence in storage/atlas.rs. docs/neural-atlas.md
    events.rs, security.rs, traits.rs, workflow_schema.rs, wait.rs
    ai.rs, cloud.rs, llm.rs, local_llm.rs, vision.rs, knowledge.rs   # experimental-facing
  platform/
    macos.rs, windows.rs, headless.rs   # OS input capture/replay backends (headless used by Linux CI)
  policy/
    capability.rs, decision.rs, engine.rs, risk.rs, zone.rs
  storage/
    mod.rs, zones.rs, executions.rs, milestones.rs, atlas.rs   # redb-backed (pure Rust, no bundled C lib); milestones.rs: local-only first-touch timing (no network); atlas.rs: durable Atlas memory graph (never deletes — archival flips a flag)
    sqlite_import.rs      # one-time SQLite -> redb migration for pre-redb installs; renames (never deletes) the legacy ghost.db
    migrations.rs         # legacy-only: brings a pre-redb ghost.db up to its final SQLite schema before sqlite_import reads it
  organizer/
    scanner.rs, classifier.rs, naming.rs, conflict.rs, planner.rs, executor.rs, undo.rs
    testutil.rs          # #[cfg(test)] temp-dir fixtures shared across organizer tests
  filing/
    mod.rs, finance.rs, academic.rs, engineering.rs, period.rs, preview.rs, savings.rs   # read-only filing preview + savings estimate across Finance/Student/Engineering audiences (see docs/filing-profiles.md)
  audit/
    audit_log.rs, undo_journal.rs
    pii.rs               # redacts SSN/card/email/phone patterns from *exported* audit text only; the stored audit log and hash chain are untouched
  checks/, compliance/, data_protection/, enterprise/, finance/, fraud/
                         # commandless domain-model scaffolding for enterprise financial-operations
                         # work (playbooks/approvals, reconciliation, KYC/AML, encryption/retention,
                         # fraud scoring, check/ID review). No Tauri commands, no network, no
                         # mutation — read docs/enterprise-financial-operations.md before adding any
                         # command surface on top of these.
```

Built so far — the Ghost Organizer trust pipeline is wired end to end:

- `policy/` — pure deny-by-default trust engine (capability/decision/risk/zone + `evaluate`); see `docs/policy-engine.md`.
- `storage/` — redb-backed Zones + folder rules + execution history + milestones. Existing installs are migrated once from the prior SQLite-backed storage on first launch after upgrade (`storage/sqlite_import.rs`); the legacy `ghost.db` is renamed, never deleted.
- `organizer/` — read-only planner (scanner/classifier/naming/conflict/planner) that emits a reviewable, policy-evaluated plan that mutates nothing (`docs/organizer-planner.md`), including invoice/receipt/statement folders, opt-in dated renaming, and the client filing preset; plus the executor and undo path (`docs/organizer-executor.md`).
- `audit/` — append-only audit log and undo journal written for every mutating run.

These are wired to the `organizer_*` Tauri commands and the Organizer UI: plan is read-only; execute re-checks policy per action, writes undo before mutating, and audits every change; undo replays the journal in reverse. The executor never overwrites or deletes silently.

Also built: Organizer audit hardening. Every executed run is sealed into a tamper-evident hash chain (`organizer_execute` persists it; `organizer_verify_audit_chain` recomputes and reports `intact`/`sealed_count`/`unsealed_count`/`first_break` offline, no network); `organizer_export_audit` returns a run's audit log as JSON/CSV with its seal metadata, and `audit/pii.rs` redacts SSN/card/email/phone patterns from that *exported* text only (the stored audit log used for the seal and undo is untouched, so redaction can never desync a run's hash). `organizer_export_policy_pack` / `organizer_import_policy_pack` move Zone + folder-rule configuration between machines, and `organizer_time_to_value` reports local first-touch milestones (first zone/plan/run/undo) for the diagnostics view.

Also built: write-ahead durability for `organizer_execute`. `executor::execute_plan_with_progress` and `storage::executions::{begin_execution, update_execution_progress, finish_execution}` mean a run's undo journal is durably updated after every action instead of held in memory and written once at the end — a crash mid-run leaves a recoverable record (`finished = 0`) of whatever had actually been applied, rather than losing it. `organizer_check_unfinished_run` surfaces an interrupted run on the next Organizer view load; the frontend offers `organizer_undo` (which also resolves it) or `organizer_dismiss_unfinished_run`. See `docs/organizer-executor.md` "Crash recovery."

Also built: Ghost Guard, the deterministic local safety layer (`core/guard.rs`, see `docs/GHOST_GUARD.md`). It suppresses keyboard capture after clicks into password/OTP/payment-shaped fields during recording, and the stable `ghost_guard_audit` / `ghost_guard_audit_compressed` commands run a pure local risk audit over raw events or the compressed semantic timeline (no network, no LLM) before replay/save. Compressed-step findings are routed into the review-timeline UI: `src/compression-review.js` runs `ghost_guard_audit_compressed` alongside `compress_workflow` and renders the Guard score and per-finding details, with a CI contract test (`src/compression-review.test.mjs`).

Also built: stable on-device OCR and ID-document parsing. `run_ocr_on_image` (`core/ocr.rs`) runs local OCR (macOS Vision / Windows OCR) over user-supplied image bytes with no network; `parse_id_document` (`core/id_scan.rs`) is a pure text-in/struct-out parser over that OCR'd text (age/expiry/review-flag signals, no image/IO/network). Both are stable core, not experimental — see `docs/core-boundaries.md`.

Also built (Ghost 2.0): the unified **Action Plan runtime** (`action_plan/` + `runtime/`). `action_plan/compile.rs` compiles either a Zone (Organizer) or recorded events (Routines) into one reviewable, policy-evaluated `ActionPlan`; `runtime/` runs the same `Capture → Review → Approve → Execute → Verify → Recover` shape over both **file** steps (`runtime/fs.rs`, the Organizer trust pipeline) and **semantic-UI** steps (`runtime/semantic.rs`/`ui.rs`, preferring Accessibility `set_value` over coordinate typing), sealing an authoritative **execution receipt** (`runtime/receipt.rs`) and writing undo. Stable commands (`commands/runtime_cmds.rs`): `action_plan_from_zone`/`_from_events`/`_demo`, `execute_action_plan`, `execute_routine_action_plan`, `get_execution_receipt`, `undo_action_plan_execution`. This is how Routines reached Organizer-grade approve→execute→verify→undo; see `docs/GHOST_2_DEMO.md`. Bare-`Allow` app/window Zones and a routine vault are still follow-up.

Also built: replay write-ahead durability + undo, mirroring Organizer's. Replay runs persist a WAL (`storage/replay_runs.rs`); `replay_check_unfinished_run` surfaces an interrupted routine on next load, and `replay_dismiss_unfinished_run` / `replay_undo` resolve it. `routine_policy_plan` maps compressed steps → `os_*` capabilities and `approve_routine_replay` + `replay_workflow` refuse a `Deny` and consume a one-shot server-issued approval token.

Also built: the **MCP surface** (`mcp/`, `docs/mcp-integration.md`, `docs/mcp-relay.md`). Stable, always-registered: device **pairing** (`mcp_pairing_status`/`_enable_pairing`/`_disable_pairing`) and an **approval queue** (`mcp_list_pending_approvals`, `organizer_issue_mcp_approval_token`) — an external MCP client can propose an action but a plan-hash-bound (`mcp/plan_hash.rs`), single-use approval token must be issued locally before anything executes through the normal trust pipeline. The HTTP server and relay transport (`mcp/http.rs`, `server.rs`, `relay.rs`, `tls.rs`; commands `mcp_start_http_server`/`_stop_http_server`/`_http_server_status`/`_relay_status`) are **experimental-gated**, so a stock build exposes pairing/approval but no listening socket.

Also built: **Ghost Atlas** (`core/atlas.rs` engine + `storage/atlas.rs` persistence, `docs/neural-atlas.md`) — a local, offline semantic-memory graph ported in concept from "Neural Atlas" but rebuilt for Ghost's trust model. Retrieval is **lexical** (character-trigram + word hashing, deterministic, reproducible) — **not** neural sentence-transformer semantics, and it must not be marketed as such (rule 10). No model download, no network, no ML runtime; "forgetting" is a reversible `archived` flag, never a delete. Commandless so far — any Tauri surface on it must still carry a module + risk class + policy + approval + audit/undo.

Also built (Ghost 2.0): the **native macOS app** (`apps/macos`, SwiftUI) and `native/macos/GhostAXHelper.swift`. SwiftUI owns scenes/navigation/review/approval/receipts UI only; the existing Rust core keeps every trust decision (scan, plan, policy, approval validation, mutation, audit, WAL, receipt, undo) and the two talk over a versioned one-JSON-object-per-line stdin/stdout bridge (`handshake`/`scan`/`create_plan`/`approve`/`execute`/`receipt`/`undo`, approval tokens signed + single-use + 5-minute). This is an implementation scaffold/vertical slice, not a shipped surface — no v2.0.x public release exercises it yet. See `docs/native-macos-preview.md`.

Also built (commandless): `checks/`, `compliance/`, `data_protection/`, `enterprise/`, `finance/`, `fraud/` are domain-model scaffolding for possible enterprise financial-operations work (playbooks/approvals/workflows, reconciliation/invoices, KYC/AML/retention, encryption/redaction/secure-delete, fraud rules/scoring/evidence, check/ID review packets). Per `docs/enterprise-financial-operations.md` these modules intentionally add **no Tauri commands, no background monitoring, no network calls, and no financial mutations** — most are still single-purpose stub files. Do not wire a command onto them without a playbook, risk class, policy check, approval step, audit/undo behavior, and an update to `docs/command-registry.md`, same as any other command.

An external repository/product audit (`docs/ghost-product-repository-audit-2026-07-11.md`) and a phased hardening handoff (`docs/post-audit-implementation-handoff.md`) are the current source of truth for release-readiness gaps. Several of those gaps have since closed (replay is policy-bound, updater signing key shipped, symlink/TOCTOU covered, Actions SHA-pinned) — see the "Status addendum — 2026-07-16" in `docs/full-repo-audit-2026-07-13.md` for verified per-finding status; Windows Authenticode signing and Guard Desk/POS Bridge marketing qualification remain live concerns. Read all of these before doing release, security, or "production readiness" framing work.

Also built: `core/compress.rs` for deterministic text compression before experimental model calls, and `core/compression/` for deterministic event compression from raw `InputEvent` streams into reviewable `CompressedStep`s. Both are exercised in product — the `compress_workflow` command and the compressed-step review timeline UI are live. Compression reports carry per-step raw-event spans (`raw_spans` + `step_for_raw_index`) so replay resolution traces map onto semantic steps: the review timeline badges steps that fell back to coordinates in the last run, history/insight rows name the semantic step, and the replay-history modal opens with a cross-run reliability summary (success rate, avg duration, coordinate-fallback share — computed in the frontend from `get_replay_history`, no new command). See `docs/token-compression.md` and `docs/event-compression.md`.

Also built: replay execution tracking and history. The engine records each replay as an `ExecutionRecord` (`core/execution.rs`); the stable `get_replay_history` command (`commands/core.rs`) exposes it, and the frontend renders a replay-history view in `src/main.js`. Organizer runs persist separately via `storage/executions.rs` and surface through `organizer_list_executions` / `organizer_undo`.

Also built: replay inspectability. Platform replay loops advance a shared `ReplayProgress` (`core/replay_support.rs`) so the stable `get_replay_progress` command can report live per-step status and the failing step index; `dry_run_workflow` (`core/dry_run.rs`) returns a pure per-step preview of what a replay would do (typed text never included). The frontend uses these for a preview modal, a live step counter, step-by-step replay, and retry-from-failed-step (the latter two replay event slices through the stable `replay_workflow` command, keeping press/release pairs together).

Also built: target resilience. Capture stores window-level locator context on `ElementInfo` (`window_title`, `window_rel` — both optional and serde-defaulted for old recordings): macOS reads the AXWindow ancestor's title and frame, Windows the GA_ROOT window's text and rect. Every replayed click records how its target resolved (`ResolutionKind`: recorded point / window-relative / spiral re-resolution / template match / coordinate fallback / no descriptor) into the run's `step_trace`, persisted on `ExecutionRecord` and rendered in the replay-history view; a post-replay insight summarizes fallback usage. The resolution chain and matcher live in `core/replay_support.rs` and are gated by the scenario benchmark in `tests/resolution_benchmark.rs` — read `docs/target-resolution.md` before changing either. Window-title-aware matching only discriminates when both sides carry titles (old recordings keep the permissive match); live window-relative re-resolution is wired on Windows (`FindWindowA`) and on macOS (libproc pid enumeration + AXWindows walk under the existing Accessibility permission — never CGWindowList, which needs Screen Recording).

Also built: pixel template-match fallback, the last resort in that same resolution chain, for elements a semantic lookup can't find (descriptor changed, or none was ever recorded) but whose pixels haven't. `core/template_match.rs` is pure Rust over the `image` crate already in the dependency tree (normalized cross-correlation, 4x downsampled, nearest-neighbor to avoid blurring template edges) — deliberately not an `opencv-rust` binding, since OpenCV needs a prebuilt system library this repo's 3-OS CI matrix doesn't install. `ElementInfo.template_png` (a small screenshot crop) is only captured when the opt-in `PerformanceSettings.capture_element_templates` is enabled (off by default — capturing a screenshot per recorded click adds latency to recording); `engine.rs::buffer_event` is the single cross-platform capture point.

Also built: account sign-in. `commands/account.rs` + `identity/oauth/flow.rs` implement "Sign in with Microsoft" / "Sign in with Google" as a public-client OAuth 2.0 + PKCE flow (no client secret): the system browser opens to the provider's consent screen, a loopback listener on an OS-assigned port receives the redirect, and the resulting profile (email/name, plus a refresh token if the provider issued one) is stored via `identity::IdentityStore` (accessed through the `accounts.rs` compatibility shim), encrypted at rest through the same `AuthManager::protect`/`reveal` envelope as workflow files whenever a vault password is configured. This is an identity link, not a data-access grant — signing in does not itself move workflow/organizer data anywhere, and it needs `integrations.microsoft_client_id`/`google_client_id` (or `GHOST_MS_CLIENT_ID`/`GHOST_GOOGLE_CLIENT_ID` for local dev) configured before it will do anything, since Ghost ships with no client IDs of its own. The Settings modal in `src/main.js` surfaces sign-in/sign-out. See `docs/integrations-roadmap.md` for how this identity is reused by the Power BI integration below and is meant to be reused by future Fabric, Google Cloud, and AI-assistant-connector integrations, none of which exist yet.

Also built (experimental): Power BI audit export. `commands/integrations.rs` (gated behind `--features experimental`, unlike account sign-in above) adds a separate, revocable `IntegrationGrant` on top of the base Microsoft identity: `power_bi_request_grant` runs incremental consent via `identity::run_grant_flow` (the same PKCE/loopback core `run_sign_in_flow` uses, factored out into a shared `authorize_and_exchange` helper, requesting the Power BI API scope instead of identity scopes and skipping the userinfo fetch), and `IdentityStore::add_grant` persists it alongside — not replacing — the identity grant. `power_bi_export_preview` is a pure, read-only command that assembles the exact `GhostRuns`/`GhostActions`/`GhostPolicyEvents` payload (`integrations/microsoft/power_bi/export.rs::build_export`) from local Organizer execution history, masking every string field through `audit::pii::mask` (the same redaction `organizer_export_audit` uses, not the separate `intelligence::redaction` module). `power_bi_push_audit_export` is the only command that touches the network: it re-derives that same payload server-side (never trusting a frontend-supplied snapshot) and pushes it via `PowerBiClient` to a dataset named `GhostOperations` in the signed-in user's own Power BI "My workspace," creating it on first use. The Settings modal's "Power BI Export" section (gated on `experimentalEnabled`, same as the AI Providers section) requires a preview to have been shown before the push button enables. There is no workspace/dataset picker yet — v1 always targets "My workspace." See `docs/power-bi-integration.md` and `docs/microsoft-auth.md`.

Also built: the **Guard Desk** view (`data-view="guard-desk"`, `Scan → Verify → Approve → POS Bridge`). It hosts the on-device check/ID OCR + verification flow and the legacy "POS Bridge" terminal auto-fill (a mock legacy terminal that types field-by-field only after the user approves Guard Desk's plan). It is explicitly labeled a **prototype desk workflow — not certified compliance**, and per `docs/full-repo-audit-2026-07-13.md` its marketing scope must stay qualified. (This view superseded the earlier `data-view="pls"` "AI Copilot" view, which no longer exists.)

The app shell should stay thin. Product logic belongs in modules that can be tested without the UI.

## Replay invariants

Do not regress these behaviors:

- clicks are press/release pairs;
- timestamps drive replay pacing;
- pause/cancel are checked inside replay loops;
- replay delays are interruptible;
- playback speed flows from engine state into platform replay;
- semantic element resolution is preferred when available;
- coordinate replay is fallback only;
- double-clicks and repeated same-position clicks are preserved.

## Event compression invariants

Do not regress these behaviors:

- press/release mouse pairs compress into one `Click` step;
- typed runs compress into one `TypeText` step;
- typed text is redacted by default;
- secure-field text is never retained, even with text retention enabled;
- shortcut chords compress into `Shortcut` with a friendly action when known;
- scroll bursts merge into one coarse `Scroll` step;
- sub-250 ms delays are dropped as noise;
- meaningful delays become `Wait` steps;
- unclassified events become `Unknown`, not silent loss;
- low-confidence and coordinate-only targets are flagged for review.

## Command surface expectations

Command modules (`src-tauri/src/commands.rs` is a thin registry over these):

- `commands/core.rs` for stable automation, recording, replay and replay history, workflow storage, permissions;
- `commands/auth.rs` for local password state and protection;
- `commands/account.rs` for Microsoft/Google OAuth account sign-in (identity only — see `docs/integrations-roadmap.md`);
- `commands/compression.rs` for the `compress_workflow` event-compression command;
- `commands/diagnostics.rs` for config, telemetry export, performance summaries, and `is_experimental_enabled`;
- `commands/organizer.rs` for the Organizer plan/execute/history/undo surface, plus audit export, signed-report verify, policy pack import/export, audit-chain verify, and time-to-value;
- `commands/runtime_cmds.rs` for the Ghost 2.0 Action Plan runtime (`action_plan_*`, `execute_action_plan`, `execute_routine_action_plan`, `get_execution_receipt`, `undo_action_plan_execution`);
- `commands/mcp.rs` for the stable MCP pairing + approval-queue surface (`mcp_pairing_status`/`_enable_pairing`/`_disable_pairing`/`_list_pending_approvals`, `organizer_issue_mcp_approval_token`); the HTTP server/relay commands stay behind `--features experimental`;
- `commands/filing.rs` for the audience-aware, read-only filing preview + savings estimate (`preview_file_filing`, `estimate_filing_savings`; audiences are Finance/Student/Engineering — see `src-tauri/src/filing/` and `docs/filing-profiles.md`);
- `commands/updates.rs` for the updater;
- `commands/intelligence.rs` (feature-gated) for internal suggestion-only OpenAI/Anthropic planning providers;
- `commands/integrations.rs` (feature-gated) for the Power BI audit-export grant flow, preview, and push (`docs/power-bi-integration.md`);
- `commands/experimental.rs` for AI, observer mode, cloud sync, analytics, visual checks, and experiments.

`commands/experimental.rs`, `commands/intelligence.rs`, `commands/integrations.rs`, and their registrations in `lib.rs` are gated behind the `experimental` Cargo feature, which is off by default. A stock build exposes only the trusted core; these commands are compiled and registered only with `--features experimental`. The frontend hides their UI (the experimental tools panel, the Settings modal's AI Providers and Power BI Export sections) unless the always-registered `is_experimental_enabled` command reports the feature is on — `src-tauri/tests/ipc_contract.rs` has focused tests asserting each gated frontend call site checks `experimentalEnabled` first. Keep new experimental commands behind this flag. CI does **not** run an experimental leg — when you touch experimental code, run the checks locally with `--features experimental` and say so in the PR (the PR template has a checkbox for it).

Before changing commands, read:

- `docs/command-registry.md`;
- `docs/core-boundaries.md`;
- `docs/organizer-commands.md` (for the Organizer IPC bridge).

Every command should document whether it touches:

- files;
- OS input;
- screenshots/screen contents;
- network;
- authentication/secrets;
- app/window state.

## Validation

Use the relevant checks:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo tauri build --no-bundle
```

Makefile shortcuts exist for all of these: `make ci` runs fmt-check + clippy + test; `make check`, `make build` (`cargo tauri build --no-bundle`), and `make dev` (`cargo tauri dev`) cover the rest.

Notes:

- the frontend is bundled by **Vite** (`vite.config.js`, root `src/`): `npm run dev` serves it at `http://localhost:1420`, `npm run build` compiles the `execution/` TypeScript then emits a CSP-clean bundle to `dist/`, and `tauri.conf.json` points `frontendDist` at `../dist` with `beforeDevCommand`/`beforeBuildCommand` wired — so `cargo tauri build`/`dev` drive Vite automatically (CI installs Node + `npm ci` first). Keep the production CSP (`script-src 'self'`) intact: the Vite config disables the inline modulepreload polyfill so the bundle stays external-only;
- local Linux builds need the GTK/webkit system deps listed in `AGENTS.md`; the `platform/headless.rs` backend is what Linux CI exercises;
- CI (`rust.yml`) runs check/test/clippy on ubuntu/macos/windows, fmt on ubuntu, and a `cargo tauri build --no-bundle` smoke test on macos/windows — it does not run `--features experimental`;
- `security.yml` runs separately (push/PR to main/develop + a weekly schedule): gitleaks secret scanning, `cargo audit`/`cargo deny check` (see `deny.toml`), and CodeQL. It does not gate PRs into `master` the way `rust.yml` does — don't assume it ran on your branch;
- integration/contract tests live in `src-tauri/tests/`: `canonical_workflows.rs`, `e2e.rs`, `frontend_dom_contract.rs`, `integration_test.rs`, `ipc_contract.rs`, `resolution_benchmark.rs` (the target-resolution scenario benchmark — see `docs/target-resolution.md` before touching it);
- for a single test, use `cargo test --manifest-path src-tauri/Cargo.toml <test_name>` (add `--features experimental` if the test lives behind the gate); for a single integration file, add `--test <file_stem>` (e.g. `--test ipc_contract`).

If checks cannot run, report that directly.

For release and signing work, read `RELEASING.md`, `docs/macos-signing-checklist.md`, and `docs/azure-signing-cost.md` before touching `release.yml`.

## Response format

When finishing a task, report:

- files changed;
- commit SHA if applicable;
- validation performed;
- risks or follow-up work.

Do not claim a build, release, signing, notarization, or CI result unless it actually happened.
