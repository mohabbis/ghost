# 👻 Ghost

An AI widget for macOS that **watches what you do, then helps you automate it.**

Ghost sits on top of macOS Accessibility. It can observe your actions (today: global
clicks), record them as a repeatable sequence, and replay them for you. The roadmap is
to layer AI reasoning on top of this observe → understand → automate loop so Ghost can
recognize repetitive work and offer to do it for you.

> Early stage. The observe/record/replay engine works; the AI layer is what comes next.

## How it works

Ghost is a [Tauri 2](https://tauri.app) desktop app with a marketing site deployable to Vercel/Netlify.

- **Frontend** (`src/`) — plain vanilla HTML/CSS/JS, no bundler. The UI for recording and
  replaying lives here and talks to Rust over Tauri IPC. Also serves as the static marketing
  site when deployed to Vercel or Netlify.
- **Backend** (`src-tauri/`) — Rust. The platform-specific integration:
  - **macOS** — `src-tauri/src/platform/macos.rs`: CGEventTap for recording, AXUIElement for
    element lookup, enigo for replay.
  - **Windows** — `src-tauri/src/platform/windows.rs`: Win32 hooks for recording, UIA for
    element lookup, enigo for replay.
  - **Engine** — `src-tauri/src/engine.rs`: Platform-agnostic orchestration with atomic
    cancellation support.

## Deployment

### Vercel (Marketing Site)

The `src/` directory contains a static site that can be deployed to Vercel:

1. Push to GitHub
2. Import project in Vercel
3. Set build command: `echo 'Static site - no build required'`
4. Set output directory: `src`

Or use the included `vercel.json` configuration.

### Netlify (Marketing Site)

Alternatively deploy to Netlify using the included `netlify.toml`:

```bash
netlify deploy --prod
```

### Tauri Desktop App

Build a distributable `.app` / `.dmg` (macOS) or `.exe` / `.msi` (Windows):

```bash
cargo tauri build
```

## Requirements

- macOS or Windows
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

## Granting Accessibility (macOS)

Ghost needs **System Settings → Privacy & Security → Accessibility** enabled for the app to
watch and replay clicks. On first run, click **Grant Access** in the UI.

> Heads up: in `cargo tauri dev` the dev binary path changes between rebuilds, so macOS may
> re-prompt or drop the permission. A stable build from `cargo tauri build` is more reliable
> for testing real recording.

## Architecture

### Core Modules

- `core/events.rs` — Shared event schema: `InputEvent`, `ElementInfo`, `KeyAction`
- `core/traits.rs` — Platform-agnostic traits: `InputRecorder`, `ElementLocator`, `ReplayEngine`
- `engine.rs` — `GhostEngine` orchestrates backends with thread-safe mpsc channels
- `platform/macos.rs` — macOS implementation using CGEventTap, AXUIElement, enigo
- `platform/windows.rs` — Windows implementation using Win32 hooks, UIA, enigo
- `commands.rs` — Tauri 2 IPC handlers with mpsc→Tauri bridge emitting `ghost:event`

### Event Flow

1. Frontend calls `start_recording()` via Tauri IPC
2. Engine spawns native recorder (CGEventTap/Win32 hook) on background thread
3. Native events flow through mpsc channel → bridge thread → `app.emit("ghost:event", payload)`
4. Frontend receives events via `listen("ghost:event", callback)`
5. Replay uses enigo with AtomicBool cancellation for instant stop

## Status

- [x] Platform-agnostic engine foundation (Phase 0)
- [x] Full macOS backend: CGEventTap, AXUIElement, enigo replay
- [x] Full Windows backend: Win32 hooks, UIA stub, enigo replay
- [x] Thread-safe mpsc bridge with atomic cancellation
- [x] Marketing site with Vercel/Netlify deployment
- [x] Interactive recording controls in frontend
- [ ] Capture *what* was clicked (AX element role/title) with full attribute extraction
- [ ] Keyboard modifier tracking and character mapping
- [ ] Scroll event phase handling
- [ ] AI layer: detect repetitive tasks and suggest automations
- [ ] Workflow persistence and library
