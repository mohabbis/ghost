# Ghost

[![Download](https://img.shields.io/badge/Download-Latest-8d7bff?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![Release](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/release.yml?style=flat-square&label=Release)](https://github.com/mohabbis/ghost/actions/workflows/release.yml)
[![macOS](https://img.shields.io/badge/macOS-12%2B-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10%2F11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

Ghost is an early-stage desktop automation app for macOS and Windows.

It records user-approved desktop actions, stores them as reusable workflows, and replays them with native input automation. The long-term goal is to make Ghost smart enough to recognize repetitive work and suggest safe automations before the user has to build them manually.

The foundation works, but this is still a technical preview. Recording and replay are useful, while reliability, debugging, cross-app robustness, signing, and AI-assisted workflow generation are still active work. Software, tragically, continues to require honesty.

## Download

| Platform | Link |
|---|---|
| macOS (Apple Silicon + Intel) | [Ghost.dmg](https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg) |
| Windows 10 / 11 (64-bit) | [Ghost_Setup.exe](https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe) |

> [!NOTE]
> The macOS build may be ad-hoc signed rather than notarized unless Apple Developer ID secrets are configured for the release workflow. If macOS blocks the app, open **System Settings → Privacy & Security** and approve it, or remove quarantine manually:
>
> ```bash
> xattr -dr com.apple.quarantine /Applications/ghost.app
> ```
>
> A notarized build is the long-term target.

## What Ghost does today

- Records desktop input events on macOS and Windows.
- Replays saved workflows using native automation.
- Stores workflows locally.
- Captures timing and basic UI element metadata where available.
- Provides Tauri IPC commands for recording, replay, workflow storage, inspection, auth, telemetry, visual checks, observer mode, and AI-assisted experiments.
- Builds as a Tauri 2 desktop app with a vanilla HTML/CSS/JS frontend and Rust backend.

## What is still experimental

Treat these as prototype or roadmap features until they are tested across real apps and documented with clear limits:

- AI workflow analysis and generation.
- Proactive observer suggestions.
- Visual regression checkpoints.
- Cloud sync and workspace management.
- Enterprise audit logging.
- Data-driven workflow testing.

## Project structure

```text
ghost/
├── src/                    # Tauri app frontend
├── public/                 # Static marketing/download site
├── src-tauri/              # Rust backend and native automation
├── docs/                   # Product and technical planning
└── .github/workflows/      # CI and release pipelines
```

Both `src/` and `public/` currently contain similar frontend assets. Keep them synchronized until the app UI and marketing site are split into separate packages.

## Architecture

Ghost is built as a Tauri 2 app.

- **Frontend:** vanilla HTML, CSS, and JavaScript. It handles recording controls, replay controls, workflow views, and Tauri IPC calls.
- **Backend:** Rust. It owns the workflow engine, platform-specific recording/replay, persistence, auth, telemetry, and command surface.
- **macOS backend:** uses native macOS accessibility/event APIs and `enigo` for replay.
- **Windows backend:** uses Win32 hooks, UI metadata lookup, and `enigo` for replay.

Core files:

- `src-tauri/src/lib.rs` — Tauri app setup and command registration.
- `src-tauri/src/commands.rs` — IPC command handlers.
- `src-tauri/src/engine.rs` — platform-agnostic orchestration.
- `src-tauri/src/core/events.rs` — shared event schema.
- `src-tauri/src/platform/macos.rs` — macOS implementation.
- `src-tauri/src/platform/windows.rs` — Windows implementation.

## Requirements

- macOS 12+ or Windows 10/11
- Rust stable
- Tauri CLI

```bash
cargo install tauri-cli --version "^2.0" --locked
```

## Development

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

Go to:

```text
System Settings → Privacy & Security → Accessibility
System Settings → Privacy & Security → Input Monitoring
```

Enable Ghost, then restart the app.

### Windows

Ghost uses Windows-native input hooks and replay APIs. Some apps running as administrator or protected system surfaces may not be controllable from a normal user-level Ghost process.

## Product roadmap

See [`docs/PRODUCT_ROADMAP.md`](docs/PRODUCT_ROADMAP.md) for the realistic plan: stable recording/replay first, workflow debugging second, constrained AI assistance third.

## Current priorities

1. Make recording and replay reliable across common apps.
2. Add a workflow debugger with per-step inspection and retry.
3. Improve semantic target resolution so workflows survive window movement.
4. Add explicit safety controls for sensitive apps and destructive actions.
5. Separate experimental AI features from stable public claims.
6. Improve release signing and installer quality.

## Release notes

The release workflow builds:

- `Ghost.dmg` for macOS.
- `Ghost_Setup.exe` for Windows.

Release packaging is intentionally separate from compile-only CI because native desktop installers fail for different reasons than Rust code. Apparently desktops are still real computers, not vibes.

## License

MIT
