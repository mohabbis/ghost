# Ghost

[![Download](https://img.shields.io/badge/Download-Latest-8d7bff?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![Release](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/release.yml?style=flat-square&label=Release)](https://github.com/mohabbis/ghost/actions/workflows/release.yml)
[![macOS](https://img.shields.io/badge/macOS-12%2B-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10%2F11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

**Record actions. Review the plan. Run the workflow. Stay in control.**

Ghost is a local-first desktop automation preview for macOS and Windows. It turns approved computer actions into reusable workflows without pretending your laptop should be possessed by a mystery agent. Low bar, somehow still rare.

```text
Record → Inspect → Approve → Replay → Audit → Undo
```

## Motion

```text
You do it once.
Ghost records it.
Ghost builds a workflow.
You review the plan.
Ghost runs it back.
```

No silent mutation. No hidden cloud brain. No autonomous chaos cosplay.

## Ghost Organizer

First wedge: clean messy folders safely.

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

Then you approve before anything moves.

## Today

Ghost currently supports:

- desktop input recording on macOS and Windows;
- native replay of saved workflows;
- local workflow storage;
- workflow inspection and management;
- timing capture and basic UI metadata where available;
- diagnostics and telemetry export paths;
- a Rust backend inside a Tauri 2 desktop app.

It is still a technical preview. Replay reliability, workflow debugging, signing, semantic targeting, and AI-assisted features are active work.

## Trust boundary

Ghost may suggest anything. It should only do what the user approves.

Risky actions must pass through the trust loop:

```text
Intent → Plan → Policy check → User approval → Execution → Audit log → Undo path
```

High-risk actions include deleting files, overwriting files, uploading files, sending messages, submitting forms, typing into unknown apps, running shell commands, using network sync, or replaying outside an approved app/folder.

## Stable vs experimental

| Stable core | Experimental |
|---|---|
| user-started recording | AI workflow analysis |
| user-started replay | AI workflow generation |
| save/load/list/delete workflows | observer mode |
| local workflow storage | proactive suggestions |
| permission checks | learned patterns |
| diagnostics | cloud sync |
| safety review paths | analytics dashboards |

Experimental features stay outside the trusted core until they have limits, tests, and user-facing controls. Revolutionary, apparently: not shipping the flamethrower before the safety switch.

## Download

| Platform | Link |
|---|---|
| macOS Apple Silicon + Intel | [Ghost.dmg](https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg) |
| Windows 10 / 11 64-bit | [Ghost_Setup.exe](https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe) |

> [!NOTE]
> Current builds are developer-preview quality. macOS builds may be ad-hoc signed unless Apple Developer ID secrets are configured.
>
> If macOS blocks the app, open **System Settings → Privacy & Security** and approve it.

## Architecture

```text
ghost/
├── src/                    # Tauri app frontend
├── public/                 # Static marketing/download site
├── src-tauri/              # Rust backend and native automation
├── docs/                   # Product and technical planning
└── .github/workflows/      # CI and release pipelines
```

- Frontend: vanilla HTML, CSS, JavaScript
- Backend: Rust
- Desktop shell: Tauri 2
- macOS: Accessibility/event APIs plus replay support
- Windows: native input hooks, UI metadata lookup, replay support

Core files:

```text
src-tauri/src/lib.rs              # app setup and command registration
src-tauri/src/commands.rs         # IPC commands
src-tauri/src/engine.rs           # platform-agnostic orchestration
src-tauri/src/core/events.rs      # shared event schema
src-tauri/src/core/security.rs    # validation and path safety
src-tauri/src/platform/macos.rs   # macOS implementation
src-tauri/src/platform/windows.rs # Windows implementation
```

## Develop

```bash
cargo install tauri-cli --version "^2.0" --locked
cargo tauri dev
```

Check the backend:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

Build:

```bash
cargo tauri build --no-bundle
cargo tauri build
```

## Permissions

### macOS

Ghost needs Accessibility permission. Keyboard capture may also require Input Monitoring.

```text
System Settings → Privacy & Security → Accessibility
System Settings → Privacy & Security → Input Monitoring
```

Restart Ghost after granting permissions.

### Windows

Ghost uses Windows-native input hooks and replay APIs. Elevated or protected apps may not be controllable from a normal user-level Ghost process.

## Priorities

1. Keep CI green.
2. Harden recording and replay.
3. Build Ghost Organizer with preview, approval, audit, and undo.
4. Add a real policy engine for dangerous actions.
5. Improve semantic targeting across app/window movement.
6. Improve signing and installer quality.
7. Keep experimental AI outside the trusted core until it earns trust.

## Rule

No feature bypasses the trust model.

Before adding a workflow capability, answer:

- What can it touch?
- Can it mutate user state?
- Can it access sensitive data?
- Can it use the network?
- Can it be undone?
- Does the user approve it first?
- Is it stable or experimental?

## License

MIT
