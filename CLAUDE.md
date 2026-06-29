# CLAUDE.md

Claude Code instructions for the Ghost repository.

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
- no cloud-first storage;
- keyboard/pointer capture only during explicit recording or approved replay.

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

Current structure:

```text
src/                    # Tauri desktop frontend
public/                 # marketing/download site
src-tauri/              # Rust backend
docs/                   # planning and technical docs
.github/workflows/      # CI and release pipelines
```

Near-term backend direction:

```text
src-tauri/src/
  commands/
    mod.rs
    core.rs
    auth.rs
    diagnostics.rs
    experimental.rs
  core/
    compress.rs          # deterministic text compression for LLM-bound content
    compression/         # deterministic event compression for workflow review
  policy/
    capability.rs
    decision.rs
    engine.rs
    risk.rs
    zone.rs
  storage/
    migrations.rs
    zones.rs
  organizer/
    scanner.rs
    classifier.rs
    naming.rs
    conflict.rs
    planner.rs
    executor.rs
    undo.rs
  audit/
    audit_log.rs
    undo_journal.rs
```

Built so far — the Ghost Organizer trust pipeline is wired end to end:

- `policy/` — pure deny-by-default trust engine (capability/decision/risk/zone + `evaluate`); see `docs/policy-engine.md`.
- `storage/` — SQLite-backed Zones + folder rules + execution history, versioned migrations.
- `organizer/` — read-only planner (scanner/classifier/naming/conflict/planner) that emits a reviewable, policy-evaluated plan that mutates nothing (`docs/organizer-planner.md`), plus the executor and undo path (`docs/organizer-executor.md`).
- `audit/` — append-only audit log and undo journal written for every mutating run.

These are wired to the `organizer_*` Tauri commands and the Organizer UI: plan is read-only; execute re-checks policy per action, writes undo before mutating, and audits every change; undo replays the journal in reverse. The executor never overwrites or deletes silently.

Also built: `core/compress.rs` for deterministic text compression before experimental model calls, and `core/compression/` for deterministic event compression from raw `InputEvent` streams into reviewable `CompressedStep`s. Both are exercised in product — the `compress_workflow` command and the compressed-step review timeline UI are live. See `docs/token-compression.md` and `docs/event-compression.md`. (Ghost Guard routing of compressed steps remains follow-up work.)

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

Command modules:

- `commands/core.rs` for stable automation, recording, replay, workflow storage, permissions;
- `commands/auth.rs` for local password state and protection;
- `commands/diagnostics.rs` for config, telemetry export, and performance summaries;
- `commands/experimental.rs` for AI, observer mode, cloud sync, analytics, visual checks, and experiments.

`commands/experimental.rs` and its registration in `lib.rs` are gated behind the `experimental` Cargo feature, which is off by default. A stock build exposes only the trusted core; the experimental commands are compiled and registered only with `--features experimental`. The frontend hides the experimental tools panel unless the always-registered `is_experimental_enabled` command reports the feature is on. Keep new experimental commands behind this flag, and run experimental checks with `--features experimental` (CI does this in a dedicated leg).

Before changing commands, read:

- `docs/command-registry.md`;
- `docs/core-boundaries.md`.

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

If checks cannot run, report that directly.

## Response format

When finishing a task, report:

- files changed;
- commit SHA if applicable;
- validation performed;
- risks or follow-up work.

Do not claim a build, release, signing, notarization, or CI result unless it actually happened.
