# Ghost

[![Download](https://img.shields.io/badge/Download-Latest-8d7bff?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![Release](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/release.yml?style=flat-square&label=Release)](https://github.com/mohabbis/ghost/actions/workflows/release.yml)
[![macOS](https://img.shields.io/badge/macOS-12%2B-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10%2F11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

Ghost is a local-first desktop automation project for macOS and Windows.

It records user-approved computer actions, stores them as reusable workflows, and replays them with native desktop automation. The long-term goal is to make repetitive computer work safe, inspectable, and eventually easier to automate through permission-bounded routines.

Ghost is still a technical preview. The current foundation works, but reliability, workflow debugging, cross-app replay, release signing, and experimental AI-assisted features are still under active development.

## Product direction

Ghost is being shaped around one principle:

> Ghost may suggest anything, but it should only do what the user has approved inside boundaries the user controls.

The stable product direction is not uncontrolled autonomous computer use. Ghost is intended to become a trusted local automation layer for repetitive computer work.

The near-term focus is:

1. reliable local recording and replay;
2. clear command and trust boundaries;
3. safe workflow review before execution;
4. permission-bounded actions;
5. auditability and undoable operations;
6. a focused file-organization workflow as the first practical product wedge.

## Download

| Platform | Link |
|---|---|
| macOS Apple Silicon + Intel | [Ghost.dmg](https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg) |
| Windows 10 / 11 64-bit | [Ghost_Setup.exe](https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe) |

> [!NOTE]
> Current release builds may still be developer-preview quality. The macOS build may be ad-hoc signed rather than fully notarized unless Apple Developer ID secrets are configured in the release workflow.
>
> If macOS blocks the app, open **System Settings → Privacy & Security** and approve it.
>
> A fully signed and notarized release is the long-term target.

## What Ghost does today

Ghost currently supports:

- recording desktop input events on macOS and Windows;
- replaying saved workflows using native automation;
- storing workflows locally;
- capturing timing and basic UI element metadata where available;
- local workflow inspection and management;
- diagnostics and telemetry export paths;
- a Rust backend inside a Tauri 2 desktop app.

The current frontend is vanilla HTML, CSS, and JavaScript. The backend is Rust.

## Stable vs experimental

Ghost's stable boundary is intentionally narrow.

### Stable core

The stable core is:

- explicit user-started recording;
- explicit user-started replay;
- workflow save, load, list, and delete;
- local workflow storage;
- local protection/auth surfaces;
- permission checks;
- diagnostics;
- safety review paths.

### Experimental surface

The following areas are experimental and should not be treated as production-ready:

- AI workflow analysis;
- AI workflow generation;
- observer mode;
- proactive suggestions;
- learned patterns;
- cloud sync;
- workspace management;
- analytics dashboards;
- visual regression checkpoints;
- data-source-driven workflow testing.

Experimental features should remain isolated from the trusted core until they have clear limits, tests, reliability coverage, and user-facing safety controls.

## Trust model

Ghost should never silently mutate important user state.

The intended execution model is:

```text
Intent → Plan → Policy check → User approval → Execution → Audit log → Undo path
```

Future development should route dangerous operations through an explicit policy layer before execution.

High-risk operations include:

- deleting files;
- overwriting files;
- uploading files;
- sending messages;
- submitting forms;
- typing into unknown apps;
- replaying actions outside an approved app or folder;
- running shell commands;
- using network/cloud sync.

## Recommended product wedge

The first practical product wedge is Ghost Organizer.

Ghost Organizer should help users clean, rename, classify, and move files safely.

Example:

```text
Downloads/
  lecture-07.pdf
  essay-rubric.pdf
  syllabus-final.pdf
```

Ghost proposes:

```text
Documents/School/BIO101/Lectures/BIO101_Lecture_07.pdf
Documents/School/ENGL201/Assignments/ENGL201_Essay_Rubric.pdf
Documents/School/CHEM110/Syllabus/CHEM110_Syllabus.pdf
```

The user should review the plan before Ghost changes anything.

The required safety behavior:

- preview first;
- no silent deletion;
- no silent overwrite;
- approval before mutation;
- audit every change;
- provide undo when possible.

## Project structure

```text
ghost/
├── src/                    # Tauri app frontend
├── public/                 # Static marketing/download site
├── src-tauri/              # Rust backend and native automation
├── docs/                   # Product and technical planning
└── .github/workflows/      # CI and release pipelines
```

`src/` and `public/` currently contain overlapping frontend/marketing assets. Keep them synchronized until the app UI and marketing site are split into separate packages.

## Architecture

Ghost is built as a Tauri 2 app.

- Frontend: vanilla HTML, CSS, and JavaScript.
- Backend: Rust.
- Desktop shell: Tauri 2.
- macOS backend: native macOS accessibility/event APIs plus replay support.
- Windows backend: Windows-native input hooks, UI metadata lookup, and replay support.

Core files:

```text
src-tauri/src/lib.rs              # Tauri app setup and command registration
src-tauri/src/commands.rs         # public IPC command registry
src-tauri/src/commands/           # grouped IPC command implementations
src-tauri/src/engine.rs           # Platform-agnostic orchestration
src-tauri/src/core/events.rs      # Shared event schema
src-tauri/src/core/security.rs    # Validation, path safety, and guard helpers
src-tauri/src/platform/macos.rs   # macOS implementation
src-tauri/src/platform/windows.rs # Windows implementation
```

## Development

Install the Tauri CLI:

```bash
cargo install tauri-cli --version "^2.0" --locked
```

Run the desktop app:

```bash
cargo tauri dev
```

Check the Rust backend:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

Compile the Tauri app without packaging installers:

```bash
cargo tauri build --no-bundle
```

Build distributable installers:

```bash
cargo tauri build
```

## Permissions

### macOS

Ghost needs Accessibility permission to observe and replay desktop actions. Keyboard capture may also require Input Monitoring.

Enable Ghost in:

```text
System Settings → Privacy & Security → Accessibility
System Settings → Privacy & Security → Input Monitoring
```

Then restart the app.

### Windows

Ghost uses Windows-native input hooks and replay APIs. Apps running as administrator or protected system surfaces may not be controllable from a normal user-level Ghost process.

## Current priorities

1. Restore and keep CI green.
2. Keep the stable command surface small and reviewable.
3. Separate experimental commands from trusted core behavior.
4. Add a real policy engine for dangerous operations.
5. Build a focused file-organization workflow with preview, approval, audit, and undo.
6. Improve replay reliability across common apps.
7. Improve semantic target resolution so workflows survive window movement.
8. Improve release signing and installer quality.

## Development rule

No new feature should bypass the trust model.

Before adding a command or workflow capability, answer:

- What can it touch?
- Can it mutate user state?
- Can it access sensitive data?
- Can it use the network?
- Can it be undone?
- Does the user approve it before execution?
- Is it stable or experimental?

## Release notes

The release workflow builds:

- `Ghost.dmg` for macOS.
- `Ghost_Setup.exe` for Windows.

Release packaging is intentionally separate from compile-only CI because native desktop installers can fail for platform-specific reasons.

## License

MIT
