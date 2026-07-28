# AGENTS.md

Canonical instructions for AI coding agents working in this repository.

Read this file before changing code, docs, workflows, releases, or product positioning. If another instruction file conflicts with this one, follow this file unless the user explicitly overrides it.

> **Product direction (current): Ghost is a cloud SaaS in `cloud/`** — the
> **trust runtime AI agents plug into** (`Agent → Ghost approve/execute/verify/audit`).
> The "Legacy desktop app" section below describes the **legacy Rust/Tauri desktop
> product** (repo root: `src-tauri/`, `src/`, `apps/macos/`) — superseded, retained
> in-tree, still accurate for that code. For the cloud product, the authoritative
> docs are `cloud/README.md`, `cloud/docs/PHASE_1_PLAN.md`,
> `cloud/docs/CURSOR_HANDOFF.md`, and `cloud/docs/AGENT_PLUGIN.md`. The trust
> *principles* in that section (deny-by-default, approval before mutation, verify,
> audit, risk classification) carry forward and remain in force for the cloud
> product; the platform specifics (Tauri commands, local-first storage,
> keyboard/replay) and the Organizer wedge do not.

## Product contract

Ghost is the **governed execution / trust runtime** delivered as a cloud SaaS
(`cloud/`): it learns a business workflow once, then executes it across software
a company already uses — browser automation and APIs today, desktop later — with
human approval on sensitive actions, verification of outcomes, and a full audit
log. External AI agents may propose and start runs via HTTP/MCP; **agents must
not approve** gated steps.

Build toward this promise:

```text
Agent (propose) -> Ghost (approve · execute · verify · audit)
Capture -> Review -> Approve -> Execute -> Verify -> Audit -> Recover
```

Prefer, in order: **APIs → browser automation → desktop automation → vision
fallback.** Ghost should understand the objective and adapt when an interface
changes — not merely replay clicks. It must not be built as a generic autonomous
agent that acts without approval on sensitive steps or without an audit trail.

Connectors (Gmail/Outlook, Salesforce, HubSpot, QuickBooks, cloud storage, etc.)
use **scoped, revocable, least-privilege credentials**, disclosed to the customer,
and every action still flows through the approval + audit pipeline below — never
around it.

## Current focus (cloud MVP)

Build the five-part MVP in `cloud/`: (1) record a browser workflow, (2) convert it
to editable steps, (3) replay across browser/API, (4) require approval before
sensitive actions, (5) log every run (status, screenshots, verification, errors).
Agent HTTP + MCP tools (no self-approve) are part of the distribution surface —
see `cloud/docs/AGENT_PLUGIN.md`.Phase 1 (the execution engine + approval gate + run UI) is built; recording is
next. See `cloud/docs/PHASE_1_PLAN.md`.

---

## Legacy desktop app (superseded — retained in-tree)

> The rest of this file documents the legacy desktop product. Read the trust
> pipeline, non-negotiable rules, and risk classes as **principles that still
> apply** to the cloud product (translate "Tauri command" → "API route / worker
> job"). Read the Organizer wedge, command architecture, build order, validation
> commands, and Cursor-Cloud notes as **desktop-specific and historical**.

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
8. Do not add cloud storage, telemetry, or network calls beyond what's already scoped and documented (account sign-in, `docs/integrations-roadmap.md`) unless the user specifically asks. Any new network surface must stay opt-in, scoped, and disclosed — never a silent default.
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

- `docs/legacy/command-registry.md` (desktop Tauri surface — legacy)
- `docs/core-boundaries.md` (cloud stable vs gated)

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
- no cloud-first storage for workflow/organizer data (it stays local and encrypted at rest regardless of which integrations are enabled);
- no unapproved network calls beyond account sign-in and explicitly-approved stack integrations (see `docs/integrations-roadmap.md`);
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

- cloud API / job / trust changes → `cloud/docs/` + `docs/trust-pipeline.md` as needed;
- product direction → `docs/product-direction.md`, `README.md`, and this file;
- desktop Tauri command changes (legacy only) → `docs/legacy/command-registry.md`;
- stable/experimental cloud boundary → `docs/core-boundaries.md`;
- Claude-specific guidance → `CLAUDE.md`;
- desktop release flow (legacy) → `RELEASING.md`;
- cloud deploy → `DEPLOYMENT.md`.

Front door: `docs/README.md`. Keep docs short and current. Delete or rewrite docs that
describe the wrong product rather than stacking more essays.

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
npm ci                        # frontend deps (Vite) — required before the build below
cargo tauri build --no-bundle # runs beforeBuildCommand (npm run build -> Vite)
```

The `src/` frontend is bundled by **Vite** (`vite.config.js`). `cargo tauri
dev`/`build` invoke `npm run dev`/`npm run build` via `beforeDevCommand`/
`beforeBuildCommand`, so `node`/`npm` and a one-time `npm install` are
prerequisites even for a Rust-only change. CI runs `npm ci` before every
`cargo tauri build`.

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

## Cursor Cloud specific instructions

The Cloud VM is Linux x86_64. Rust stable (via `rust-toolchain.toml`), the Tauri
CLI, and all system libs are preinstalled by the environment; the startup update
script only refreshes crate deps (`cargo fetch`) and ensures the Tauri CLI is
present. Standard commands are already documented above (`README.md`, `Makefile`,
`## Validation`) — use those. Only the non-obvious Linux gotchas are captured here:

- **Running the desktop app in dev mode:** `cargo tauri dev` fails with
  "`cargo run` could not determine which binary to run" because the crate exposes
  multiple bins (`ghost`, `diagnose_perms`, and gated `mcp_relay_server`) and sets
  no `default-run`. Pass the bin through to the runner:
  `cargo tauri dev -- --bin ghost` (Makefile's `make dev` hits the same ambiguity).
  `cargo tauri build --no-bundle` is unaffected (it builds all bins).
- **GUI testing:** an X11 display is available at `DISPLAY=:1`. The app renders
  there via `webkit2gtk` with software rendering; `libEGL warning: DRI3 ...`
  messages are harmless. Ghost's macOS/Windows input capture/replay is unavailable
  on Linux (only `platform/headless.rs` compiles), but the Ghost Organizer flow
  (Zone -> add folder -> Scan & Preview -> Approve & Organize -> History -> Undo)
  works fully and is the best end-to-end smoke test. The Organizer requires
  creating a Zone and adding the target folder before Scan & Preview is meaningful.
- **Extra system dep vs. CI:** beyond the GTK/webkit libs listed above, a bare VM
  also needs `libssl-dev` + `pkg-config` (for `openssl-sys`). GitHub's CI runners
  ship these preinstalled, so they are absent from `rust.yml`'s apt list.

## Final response expectations

When reporting work back to the user, include:

- files changed;
- commit SHA if pushed;
- checks run or not run;
- known risks or follow-up work.

Do not claim checks, builds, releases, signing, notarization, or installer validation unless they actually happened.

## Cursor Cloud specific instructions

This section captures non-obvious caveats for running Ghost inside a Cursor Cloud VM (Ubuntu 24.04, no macOS/Windows). Standard build/test/lint commands are already documented above (see "Validation") and in the `Makefile`/`README.md` — use those; only the caveats below are cloud-specific.

- **Toolchain is pre-provisioned by the startup update script.** Rust stable (pinned by `rust-toolchain.toml`), `cargo-tauri` (installed via `cargo install tauri-cli`), the Linux GTK/WebKit system libs (`libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libxdo-dev`), and `libssl-dev`/`pkg-config` are baked into the VM image. `libssl-dev` is required even for a default (non-experimental) build because a transitive dependency pulls `openssl-sys`. If a build fails on missing GTK/WebKit/openssl, re-run the corresponding `apt-get install`.

- **Running the app needs an explicit binary.** The crate ships three binaries (`ghost`, `diagnose_perms`, `mcp_relay_server`) and has no `default-run`, so plain `cargo tauri dev` / `make dev` fails with "could not determine which binary to run". Launch the desktop app with `cargo tauri dev -- --bin ghost` (the args after `--` are forwarded to the cargo runner).

- **GUI runs on the VNC desktop with software rendering.** There is an XFCE desktop on `DISPLAY=:1` (TigerVNC) that the computer-use tooling drives. WebKitGTK renders blank under this headless/VNC GPU stack unless you disable hardware acceleration. Export these before launching: `DISPLAY=:1 WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 LIBGL_ALWAYS_SOFTWARE=1`. Without them the window opens but stays black.

- **Organizer works headlessly, no permissions needed.** The Ghost Organizer wedge (Zone → Scan → Approve → Organize → Audit → Undo) operates purely on the local filesystem and does not need macOS Accessibility/Input Monitoring grants, so it is the reliable end-to-end flow to exercise on Linux. Record/Replay uses the `platform/headless.rs` backend on Linux (no real input capture).

- **Frontend Node test must target the file, not the dir.** Run `node --test src/compression-review.test.mjs`. `node --test src/` fails with `MODULE_NOT_FOUND` because of how the sibling module is imported.

- **Experimental surface is off by default.** Add `--features experimental` to build/run the AI/Power BI/MCP-relay code. `tokenizers` is declared without the default `esaxx_fast` feature so the experimental local-model path does not compile `esaxx-rs` C++ (which fails on this VM when `/usr/bin/c++` is clang selecting GCC 14 without `libstdc++-14-dev`). If a future dep reintroduces a C++ build and you see `'cstdint' file not found`, either `export CXX=g++ CC=gcc` or `sudo apt-get install -y libstdc++-14-dev`. `rust.yml` has an experimental check/test/clippy leg; still run `--features experimental` locally when you touch that surface.
