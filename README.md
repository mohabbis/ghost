# 👻 Ghost

An AI widget for macOS that **watches what you do, then helps you automate it.**

Ghost sits on top of macOS Accessibility. It can observe your actions (today: global
clicks), record them as a repeatable sequence, and replay them for you. The roadmap is
to layer AI reasoning on top of this observe → understand → automate loop so Ghost can
recognize repetitive work and offer to do it for you.

> Early stage. The observe/record/replay engine works; the AI layer is what comes next.

## How it works

Ghost is a [Tauri 2](https://tauri.app) desktop app.

- **Frontend** (`src/`) — plain vanilla HTML/CSS/JS, no bundler. The UI for recording and
  replaying lives here and talks to Rust over Tauri IPC.
- **Backend** (`src-tauri/`) — Rust. The macOS integration is in `src-tauri/src/macos_ax.rs`:
  - **Permission** — checks/requests macOS Accessibility access.
  - **Record** — a `CGEventTap` listens for global left-clicks and streams each one to the
    UI via the `ghost:click-captured` event.
  - **Replay** — synthesizes clicks at the recorded coordinates with [`enigo`](https://crates.io/crates/enigo).

Everything in the engine is macOS-only by design.

## Requirements

- macOS
- [Rust](https://rustup.rs) (stable)
- [Tauri CLI](https://tauri.app/start/) — `cargo install tauri-cli`

## Develop

```bash
cargo tauri dev          # run the app (Vite-less; serves src/ directly)
```

Or work with the backend directly:

```bash
cd src-tauri
cargo check              # fast type-check
cargo clippy             # lint
cargo build --release    # build the library
```

Build a distributable `.app` / `.dmg`:

```bash
cargo tauri build
```

## Granting Accessibility

Ghost needs **System Settings → Privacy & Security → Accessibility** enabled for the app to
watch and replay clicks. On first run, click **Grant Access** in the UI.

> Heads up: in `cargo tauri dev` the dev binary path changes between rebuilds, so macOS may
> re-prompt or drop the permission. A stable build from `cargo tauri build` is more reliable
> for testing real recording.

## Status

- [x] Accessibility permission gating
- [x] Global click recording (`CGEventTap` → `ghost:click-captured`)
- [x] Click replay
- [ ] Capture *what* was clicked (AX element role/title), not just coordinates
- [ ] Keyboard + scroll capture
- [ ] AI layer: detect repetitive tasks and suggest automations
