# AGENTS.md

Guidance for coding agents working in this repository.

## Mission

Ghost is a local-first automation layer for repetitive computer work. Build toward trust, not spectacle.

The product should start with Ghost Organizer: safe file organization with folder boundaries, preview, approval, audit, and undo. Broad app automation, AI-generated workflows, observer mode, and cloud features are experimental until the policy and trust model are real.

## Non-negotiable product rules

1. Do not present Ghost as a generic autonomous agent.
2. Do not build hidden or always-on observation features.
3. Do not request camera or microphone permissions.
4. Do not read email, browser, or document contents unless the user explicitly grants a scoped action.
5. Do not let AI output directly execute operations.
6. Do not silently overwrite or delete files in the MVP.
7. Do not add new Tauri commands without assigning a module and risk class.
8. Do not move experimental features into default UI without documented limits and tests.

Canonical pipeline:

```text
Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
```

Every meaningful operation should fit this pipeline.

## Current command architecture

Command implementations are split by product boundary:

- `src-tauri/src/commands/core.rs`
- `src-tauri/src/commands/auth.rs`
- `src-tauri/src/commands/diagnostics.rs`
- `src-tauri/src/commands/experimental.rs`

`src-tauri/src/commands.rs` should stay a thin registry/re-export surface. Do not turn it back into an 800-line drawer of doom. The world has enough drawers.

See `docs/command-registry.md` and `docs/core-boundaries.md` before changing command surfaces.

## Stable vs experimental

Stable core:

- permissions
- explicit recording
- replay controls
- workflow storage
- event review
- element inspection
- local auth/protection
- diagnostics
- safety audit concepts

Experimental:

- AI workflow generation
- observer suggestions
- cloud sync
- workspaces
- analytics dashboards
- visual regression
- data-source workflow testing
- broad proactive intelligence

Experimental code may exist, but it must stay separated, gated, or clearly labeled.

## Privacy defaults

Default product posture:

- no camera
- no microphone
- no hidden screen capture
- no background email or draft monitoring
- no unscoped browser/tab reading
- no raw secret capture
- no cloud-first storage

Browser and app integrations must be scoped by user-approved Zones, windows, tabs, domains, folders, and actions.

## Build priorities

Work in this order unless explicitly instructed otherwise:

1. Command risk inventory.
2. Policy engine skeleton.
3. Zones and folder rules.
4. Ghost Organizer planner.
5. Plan preview and review UI.
6. Organizer executor with audit and undo.
7. Release signing/notarization path.
8. Recorded routines after Organizer proves the trust model.
9. AI suggestions last, suggestion-only.

## Validation

Run relevant checks before claiming code is ready:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo tauri build --no-bundle
```

If the environment cannot run checks, say that plainly and leave a validation note. Do not perform the ancient ritual of pretending CI passed because vibes were positive.
