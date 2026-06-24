# CLAUDE.md

This file guides Claude Code when working in the Ghost repository.

## Product identity

Ghost is a local-first automation layer for repetitive personal computer work.

Do not frame Ghost as a generic AI agent, chatbot, macro recorder, RPA clone, or app that takes over the user's computer. The product value is trustworthy execution: clear boundaries, reviewable plans, deterministic operations, audit logs, and undo paths.

Canonical positioning:

> Ghost turns repeated computer work into safe, reusable, permission-bounded routines.

The strategic wedge is Ghost Organizer: safe file organization with preview, approval, folder boundaries, audit, and undo.

## Current repo reality

Ghost is currently an early-stage Tauri 2 desktop app for macOS and Windows. It has foundations for recording, replay, workflow storage, local protection, diagnostics, and platform-specific automation. It is not ready to be marketed as a broad computer-controlling assistant.

Stable product center:

- permission checks and requests
- explicit user-approved recording
- replay controls: cancel, pause, resume, speed
- workflow save/load/list/delete
- recorded event review and element inspection
- local auth and at-rest workflow protection
- diagnostics and safe telemetry export
- Ghost Guard / safety audit concepts

Experimental unless hardened and feature-gated:

- AI workflow analysis and optimization
- prompt-generated workflows
- proactive observer mode
- learned-pattern suggestions
- cloud sync and workspaces
- enterprise audit logs
- analytics dashboards
- visual regression workflows
- data-source workflow testing
- geek insights

Do not add new user-facing UI for experimental surfaces unless the task explicitly calls for developer/experimental mode.

## Core engineering philosophy

Every meaningful operation should pass through this pipeline:

```text
Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
```

Rules:

1. No file, app, window, network, or user-data operation should bypass policy.
2. AI output may propose, but must not execute directly.
3. Every meaningful operation must write an audit event.
4. Every reversible operation must write an undo entry before execution.
5. Ghost must deny by default.
6. Experimental features must be isolated behind explicit flags or modules.
7. Coordinates are fallback automation identity, never the primary model.
8. No Tauri command should be added without a risk classification.
9. The MVP must not silently overwrite or delete files.
10. Human approval is required before meaningful changes.

AI may suggest plans, categories, filenames, explanations, routine drafts, and improvements. The deterministic Ghost core must verify and execute only approved plans.

## Privacy and permission boundaries

Ghost should not be always watching.

Default stance:

- No camera requirement.
- No microphone requirement.
- No hidden screen capture.
- No background email or draft monitoring.
- Keyboard and pointer capture only during explicit recording or approved routine execution.
- Browser and email access only when user-approved, scoped, and zone-bound.

For Ghost Organizer MVP, avoid broad OS permissions where possible. File organization should work through explicit folder selection and local filesystem operations.

For Ghost Routines later, input recording may be used only with visible active state, emergency stop, permission checks, and sensitive-input suppression.

Browser integration should evolve in layers:

1. OS-level active app/window awareness.
2. Browser extension for active-tab/domain metadata and explicit selected-text sharing.
3. Official APIs where appropriate.

Per-tab or per-window access must be scoped through Zones and app/domain rules.

## Target product layers

Design Ghost as three coherent layers sharing one trust engine:

1. Ghost Organizer
   - safe file/folder cleanup
   - classification, naming, moving, conflicts, preview, audit, undo
2. Ghost Routines
   - explicit recorded routines across apps and websites
   - semantic replay with coordinates as fallback
3. Ghost Intelligence
   - suggestion-only planning, explanation, classification, and routine detection

Build order: Organizer first, Routines second, Intelligence last.

## Architecture direction

Keep Rust/Tauri for now. Do not rewrite the whole product before proving the wedge.

Near-term refactor direction:

```text
src-tauri/src/
  commands/
    mod.rs
    core.rs
    auth.rs
    diagnostics.rs
    experimental.rs
  policy/
    capability.rs
    engine.rs
    risk.rs
  organizer/
    scanner.rs
    classifier.rs
    planner.rs
    executor.rs
    undo.rs
  audit/
    audit_log.rs
    undo_journal.rs
```

Longer-term monorepo direction:

```text
apps/desktop/
crates/ghost-core/
crates/ghost-policy/
crates/ghost-organizer/
crates/ghost-workflows/
crates/ghost-platform/
crates/ghost-platform-macos/
crates/ghost-platform-windows/
crates/ghost-storage/
crates/ghost-security/
crates/ghost-audit/
crates/ghost-ai/
```

The app shell should stay thin. Product logic belongs in modules/crates that can be tested without the UI.

## Current app structure

The current desktop app uses:

- `src/` for the Tauri desktop frontend.
- `public/` for the marketing/download site.
- `src-tauri/` for the Rust backend.
- `docs/` for product and technical planning.
- `.github/workflows/` for CI and release pipelines.

Important: `src/` and `public/` are not interchangeable. The desktop UI and marketing site may share assets or tokens, but do not blindly sync their HTML/JS behavior.

## Replay invariants

Do not regress these behaviors:

- Clicks are press/release pairs.
- Timestamps drive replay pacing and must be preserved when events are transformed.
- Pause/cancel must be checked inside replay loops.
- Use interruptible sleeps for replay delays.
- Playback speed must flow from engine state into platform replay.
- Semantic element resolution should be preferred when available.
- Coordinate replay is fallback only.
- Double-clicks and repeated same-position clicks must not be debounced away.

## Command surface expectations

Command modules should be grouped by intent:

- stable core
- auth/protection
- diagnostics
- experimental

Every command should have a documented risk class. At minimum, track whether it touches:

- files
- OS input
- screenshots/screen contents
- network
- authentication/secrets
- app/window state

High and critical commands must require explicit approval, remain developer-only, or be unavailable in default product UI.

## Ghost Organizer MVP

The next serious product slice is Ghost Organizer.

MVP flow:

1. User selects a source folder.
2. User selects a destination folder or Zone.
3. User defines categories or chooses templates.
4. Ghost scans candidate files.
5. Ghost classifies deterministically first.
6. Ghost generates safe target folders and filenames.
7. Ghost detects conflicts and low-confidence items.
8. Ghost creates a reviewable plan.
9. Policy engine evaluates the plan.
10. User approves.
11. Executor performs deterministic filesystem operations.
12. Audit log and undo journal are written.
13. User sees completion summary and undo option.

MVP must not delete files. It must not silently overwrite. It must keep operations inside approved folder boundaries.

## Testing and validation

Before declaring a feature ready, run when applicable:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo tauri build --no-bundle
```

For release readiness, do not call Ghost consumer-ready until:

- macOS builds are Developer ID signed and notarized.
- Windows builds are signed.
- experimental commands are hidden, gated, or clearly labeled.
- app UI and marketing/download site are separated or generated from a shared source.
- workflow schemas have versioning and migration tests.
- file operations have policy, audit, and undo coverage.
