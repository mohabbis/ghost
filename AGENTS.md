# AGENTS.md

Canonical instructions for AI coding agents working in this repository.

Read this file before changing code, docs, workflows, releases, or product positioning. If another instruction file conflicts with this one, follow this file unless the user explicitly overrides it.

## Product contract

Ghost is a local-first desktop automation product for macOS and Windows.

Build toward this promise:

```text
Record -> Inspect -> Approve -> Replay -> Audit -> Undo
```

Ghost should turn repeated computer work into safe, reusable, permission-bounded routines. It must not be presented or built as a generic autonomous agent that silently controls the user's computer.

## Current product wedge

Prioritize **Ghost Organizer** first.

Ghost Organizer helps users organize files safely:

```text
Select folder -> Scan -> Propose plan -> Review -> Approve -> Move/Rename -> Audit -> Undo
```

Required behavior:

- user chooses the source folder;
- user chooses destination boundaries or a Zone;
- Ghost previews every move/rename before execution;
- Ghost never silently deletes files;
- Ghost never silently overwrites files;
- Ghost detects conflicts and low-confidence items;
- user approves before mutation;
- executor writes an audit log;
- reversible operations write undo data before execution.

Broad app automation, AI-generated workflows, observer mode, cloud sync, analytics, and proactive suggestions are experimental until the trust model is implemented and tested.

## Non-negotiable rules

1. Do not build hidden or always-on observation.
2. Do not request camera or microphone permissions.
3. Do not read email, browser, document, or screen contents unless the user explicitly grants a scoped action.
4. Do not let AI output directly execute operations.
5. Do not silently overwrite, delete, upload, send, submit, or type into unknown apps.
6. Do not add a Tauri command without a module, owner boundary, and risk class.
7. Do not move experimental features into default UI without documented limits, tests, and user-facing controls.
8. Do not weaken local-first behavior by adding cloud storage, telemetry, or network calls unless the user specifically asks.
9. Do not claim production readiness if release signing, replay reliability, policy checks, or undo paths are incomplete.
10. Do not make README or marketing copy promise more than the repo can support.

## Trust pipeline

Every meaningful operation must fit this pipeline:

```text
Intent -> Plan -> Policy check -> User approval -> Execution -> Audit log -> Undo path
```

Definitions:

- **Intent:** what the user asked Ghost to do.
- **Plan:** exact proposed actions before mutation.
- **Policy check:** risk classification, allowed scope, conflicts, and denied operations.
- **User approval:** explicit approval for the final plan.
- **Execution:** deterministic operation, not raw AI output.
- **Audit log:** durable record of what changed.
- **Undo path:** instructions or journal entries to reverse the change where possible.

## Risk classes

Use these classes when adding or reviewing commands, workflows, and UI actions.

| Class | Meaning | Examples | Default behavior |
|---|---|---|---|
| `safe-read` | Reads local metadata without sensitive content | list workflows, read app settings | allowed with normal UI action |
| `sensitive-read` | Reads user content or potentially private data | file contents, selected text, window titles, screenshots | require scoped permission and visible state |
| `local-mutate` | Changes local state | move file, rename file, delete workflow | require plan, approval, audit, undo when possible |
| `external-mutate` | Sends data or changes remote state | upload file, send message, submit form, cloud sync | deny by default; require explicit feature scope |
| `os-control` | Controls input or other apps | replay clicks, type text, run shell command | require approved target scope and emergency stop |
| `experimental` | Not trusted product behavior yet | AI generation, observer suggestions, visual regression | keep gated, labeled, and out of default UI |

When unsure, classify the action as the higher-risk class. Optimism is not a security model.

## Command architecture

Command implementations are split by product boundary:

- `src-tauri/src/commands/core.rs`
- `src-tauri/src/commands/auth.rs`
- `src-tauri/src/commands/diagnostics.rs`
- `src-tauri/src/commands/experimental.rs`

`src-tauri/src/commands.rs` should stay a thin registry/re-export surface.

Before adding or changing commands, read:

- `docs/command-registry.md`
- `docs/core-boundaries.md`

Command requirements:

- assign a module;
- assign a risk class;
- document input scope;
- document mutation behavior;
- document whether approval is required;
- add or update tests for valid, invalid, interrupted, and denied flows where practical.

## Stable vs experimental

### Stable core

Stable core work may define product behavior:

- permissions;
- explicit recording;
- replay controls;
- workflow storage;
- event review;
- element inspection;
- local auth/protection;
- diagnostics;
- policy checks;
- audit and undo primitives;
- Ghost Organizer plan/review/execute flow.

### Experimental

Experimental work must stay gated, isolated, or clearly labeled:

- AI workflow generation;
- AI workflow analysis;
- observer suggestions;
- learned patterns;
- cloud sync;
- workspaces;
- analytics dashboards;
- visual regression;
- data-source workflow testing;
- broad proactive intelligence.

AI can suggest plans, categories, filenames, explanations, and routine drafts. Deterministic Ghost code must verify and execute only approved plans.

## Privacy defaults

Default posture:

- no camera;
- no microphone;
- no hidden screen capture;
- no background email monitoring;
- no background browser/tab reading;
- no raw secret capture;
- no cloud-first storage;
- no unapproved network calls;
- keyboard and pointer capture only during explicit recording or approved replay.

Browser and app integrations must be scoped by user-approved Zones, windows, tabs, domains, folders, and actions.

## Build order

Work in this order unless the user explicitly changes priorities:

1. Keep CI green.
2. Keep command surfaces small and classified.
3. Add/maintain policy engine primitives.
4. Add/maintain Zones and folder rules.
5. Build Ghost Organizer scanner/planner.
6. Build plan preview and review UI.
7. Build deterministic executor with audit and undo.
8. Improve replay reliability and semantic targeting.
9. Improve release signing and installer quality.
10. Add AI suggestions last, suggestion-only, behind clear gates.

## Documentation rules

When changing behavior, update the matching docs in the same change:

- command changes -> `docs/command-registry.md`;
- stable/experimental boundary changes -> `docs/core-boundaries.md`;
- product direction changes -> `README.md` and this file;
- Claude-specific guidance -> `CLAUDE.md`;
- release flow changes -> `RELEASING.md`;
- deployment changes -> `DEPLOYMENT.md` or `VERCEL_DEPLOYMENT_GUIDE.md`.

Keep docs short, current, and operational. Do not add vision essays when a checklist will do.

## Working in parallel (multiple agents)

This repo is often worked by several agents/sessions at once. To avoid
colliding and to keep `master` green:

- Sync first: `git fetch origin master` and rebase/reset your branch onto it
  before starting and again before pushing. Never branch from a stale base.
- One change per PR. Keep PRs small and single-purpose so they review and
  merge fast and rarely conflict.
- Localize edits to the conflict-hotspot files. `src-tauri/src/lib.rs` (the
  `generate_handler!` list) and `src/main.js` are touched by almost every
  feature — add your entry in one place, don't reflow surrounding code.
- Run the validation commands below before claiming done. Local Linux builds
  need the GTK/webkit system deps (`libgtk-3-dev libwebkit2gtk-4.1-dev
  libappindicator3-dev librsvg2-dev patchelf libxdo-dev`); CI installs them.
- Respect the boundaries: new experimental commands stay behind the
  `experimental` Cargo feature; don't edit `.github/workflows/*` unless your
  token has `workflow` scope (the push will be rejected otherwise).
- Tooling is pinned (`rust-toolchain.toml`) and whitespace is normalized
  (`.editorconfig`, `.gitattributes`, LF) so diffs stay minimal across
  platforms — don't fight them with reformat-only changes.

## Validation

Run relevant checks before claiming code is ready:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo tauri build --no-bundle
```

If the environment cannot run checks, say that plainly and leave a validation note.

### Faster local dev builds

`[profile.dev]` in `src-tauri/Cargo.toml` pins `incremental = true` explicitly.
For a faster linker, opt in locally with a `src-tauri/.cargo/config.toml`
(not checked in — mold/lld availability varies per machine and CI doesn't
install either, so this is not a repo-wide default):

```toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]  # or mold, if installed
```

## Final response expectations

When reporting work back to the user, include:

- files changed;
- commit SHA if pushed;
- checks run or not run;
- known risks or follow-up work.

Do not claim checks, builds, releases, signing, notarization, or installer validation unless they actually happened.
