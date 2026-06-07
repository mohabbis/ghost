# 👻 Ghost

[![Latest Release](https://img.shields.io/github/v/release/mohabbis/ghost?style=flat-square&label=Download&color=8d7bff)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![Release](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/release.yml?style=flat-square&label=Release)](https://github.com/mohabbis/ghost/actions/workflows/release.yml)
[![macOS](https://img.shields.io/badge/macOS-12%2B-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10%2F11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

Your **smart AI parrot helper/geek** — an intelligent desktop companion that watches what you do, learns your patterns, and proactively helps with repetitive tasks.

Like a clever parrot, Ghost observes your behavior silently and pops up with suggestions: *"Hey, I noticed you copy-paste customer info every morning — want me to automate that?"* It's more than just recording/replaying inputs; it's an active assistant that understands your workflow and offers help before you even ask.

## Download

| Platform | Link |
|---|---|
| 🍎 macOS (Apple Silicon + Intel) | [**Ghost.dmg**](https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg) |
| 🪟 Windows 10 / 11 (64-bit) | [**Ghost_Setup.exe**](https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe) |

> [!NOTE]
> The macOS build is **ad-hoc signed, not notarized**, so Gatekeeper blocks it on first launch
> ("Apple could not verify…"). On macOS 15 (Sequoia) the old right-click → Open trick no longer
> clears this dialog. To run it:
>
> 1. Drag **ghost** into `Applications`, then in Terminal run:
>    `xattr -dr com.apple.quarantine /Applications/ghost.app`
> 2. Or open **System Settings → Privacy & Security**, scroll to the "ghost was blocked" notice,
>    and click **Open Anyway**.
>
> This is expected for unsigned apps and does not mean the app is unsafe. Only a notarized build
> (paid Apple Developer ID) opens with no prompt — see [RELEASING.md](RELEASING.md).

## 📁 Project Structure

```
ghost/
├── src/                    # Tauri app frontend (desktop application)
├── public/                 # Marketing website (ghost.muharafiq.com)
├── src-tauri/             # Rust backend
└── .github/workflows/     # CI/CD pipelines
```

**Important:** Both `src/` and `public/` contain identical HTML/CSS/JS files:
- `src/` is used by the Tauri desktop app
- `public/` is deployed to the marketing website
- Keep both directories synchronized when making UI changes

See [DEPLOYMENT.md](DEPLOYMENT.md) for detailed deployment instructions.

## What is Ghost?

Ghost is your **smart AI parrot** — an intelligent assistant that:
- **Observes** your desktop activity silently, learning your unique patterns
- **Learns** what workflows you repeat and when you do them
- **Assists** proactively with "Hey, I noticed you..." style suggestions
- **Automates** repetitive tasks so you can focus on what matters

It sits on top of macOS Accessibility (and Windows UIA) to understand your clicks, keystrokes, and workflow intent — then turns those actions into reliable automations.

## How it helps (like a smart parrot)

1. **Observe** — Your parrot silently watches your desktop activity, learning your unique patterns
2. **Learn** — It builds understanding of your intent and recognizes when you repeat actions  
3. **Assist** — Pops up with proactive suggestions: "Hey, I noticed you ___"

- **Frontend** (`src/`) — plain vanilla HTML/CSS/JS, no bundler. The UI for recording and
  replaying lives here and talks to Rust over Tauri IPC. Also serves as the static marketing
  site when deployed to Vercel or Netlify, featuring the smart AI parrot demo.
- **Backend** (`src-tauri/`) — Rust. The platform-specific integration:
  - **macOS** — `src-tauri/src/platform/macos.rs`: CGEventTap for recording, AXUIElement for
    element lookup, enigo for replay.
  - **Windows** — `src-tauri/src/platform/windows.rs`: Win32 hooks for recording, UIA for
    element lookup, enigo for replay.
  - **Engine** — `src-tauri/src/engine.rs`: Platform-agnostic orchestration with atomic
    cancellation support.

## Deployment

### Marketing Website

The marketing website is hosted at [ghost.muharafiq.com](https://ghost.muharafiq.com) and serves the `public/` directory. Download links automatically point to the latest GitHub Release assets.

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

All Phase 4 features are now in production on `master`:

- [x] Platform-agnostic engine foundation (Phase 0)
- [x] Full macOS backend: CGEventTap, AXUIElement, enigo replay with speed control
- [x] Full Windows backend: Win32 hooks, UIA, enigo replay with speed control  
- [x] Thread-safe mpsc bridge with atomic cancellation
- [x] Marketing site with Vercel/Netlify deployment
- [x] Interactive recording controls in frontend
- [x] Workflow save/load functionality
- [x] Playback speed control (0.1x - 2.0x+)
- [x] Pause/resume replay functionality
- [x] AI-powered workflow analysis and optimization
- [x] Workflow naming suggestions using pattern detection
- [x] Reliability replay with configurable retry and backoff strategies
- [x] Cloud sync capabilities with authentication
- [x] Workspace management for team collaboration
- [x] Enterprise audit logging for compliance
- [x] Accessibility permission handling (check/request)
- [x] Real-time event timeline visualization
- [x] **Smart AI Parrot UI** - Interactive parrot avatar with typing animation, clickable to see proactive suggestions
- [x] **Marketing site** - Shows both macOS and Windows support with app mockup
- [x] **Phase 4A: Visual Regression** - Visual checkpoints during replay with SSIM comparison, baseline capture, mismatch handling
- [x] **Phase 4B: Smart Observer Mode** - Watches/learns patterns, proactive suggestions, pattern detection
- [x] **Phase 4C: Data-Driven Testing** - CSV/JSON/Environment data sources, template resolution
- [x] **Phase 4D: Geek Mode Insights** - Technical performance metrics, event timing analysis for power users
- [x] **Phase 5: Execution & Analytics** - Execution history tracking, workflow analytics
- [ ] Capture *what* was clicked (AX element role/title) with full attribute extraction
- [ ] Keyboard modifier tracking and character mapping
- [ ] Scroll event phase handling
- [ ] Cross-platform desktop deployment

## API Reference

### Recording Commands

| Command | Description | Parameters |
|---------|-------------|------------|
| `start_recording` | Begin capturing input events | `app: AppHandle`, `engine: GhostEngine` |
| `stop_recording` | Stop the active recording session | `engine: GhostEngine` |
| `get_recorded_events` | Get all events from current session | `engine: GhostEngine` |

### Playback Commands

| Command | Description | Parameters |
|---------|-------------|------------|
| `replay_workflow` | Execute a sequence of events | `events: Vec<InputEvent>`, `engine: GhostEngine` |
| `cancel_replay` | Immediately stop ongoing replay | `engine: GhostEngine` |
| `pause_replay` | Pause a running replay | `engine: GhostEngine` |
| `resume_replay` | Resume a paused replay | `engine: GhostEngine` |
| `is_replay_paused` | Check if replay is paused | `engine: GhostEngine` |
| `is_replay_running` | Check if replay is active | `engine: GhostEngine` |
| `set_playback_speed` | Set speed factor (0.1x-2.0x+) | `factor: f32`, `engine: GhostEngine` |
| `get_playback_speed` | Get current speed factor | `engine: GhostEngine` |

### Workflow Management

| Command | Description | Parameters |
|---------|-------------|------------|
| `save_workflow` | Save events to JSON file | `name: String`, `events: Vec<InputEvent>`, `engine: State<GhostEngine>` |
| `load_workflow` | Load events from JSON file | `name: String`, `engine: State<GhostEngine>` |
| `delete_workflow` | Remove workflow from disk | `name: String`, `engine: State<GhostEngine>` |
| `list_workflows` | List all saved workflow names | Returns `Vec<String>` |
| `save_workflow_with_metadata` | Save workflow with description/tags | `name`, `events`, `description`, `tags`, `engine` |
| `load_workflow_with_metadata` | Load complete workflow object | `name: String`, `engine: State<GhostEngine>` |

### AI-Powered Commands

| Command | Description | Parameters |
|---------|-------------|------------|
| `analyze_workflow` | Get AI insights about a workflow | `name: String`, `events: Vec<InputEvent>`, `engine: State<GhostEngine>` |
| `optimize_workflow` | Generate optimized event sequence | `events: Vec<InputEvent>`, `engine: State<GhostEngine>` |
| `suggest_workflow_name` | Generate name from event patterns | `events: Vec<InputEvent>`, `engine: State<GhostEngine>` |

### Reliability Commands

| Command | Description | Parameters |
|---------|-------------|------------|
| `replay_with_reliability` | Execute with retry/backoff/checkpoints | `events`, `max_attempts`, `backoff_ms`, `backoff_multiplier`, `checkpoints`, `engine` |

### Cloud Sync Commands

| Command | Description | Parameters |
|---------|-------------|------------|
| `init_cloud_sync` | Initialize cloud manager | `config: CloudConfig`, `state: State<CloudState>` |
| `cloud_authenticate` | Login with auth token | `token: String`, `state: State<CloudState>` |
| `cloud_sync_workflows` | Sync workflows to cloud | `name`, `events`, `description`, `state` |
| `create_workspace` | Create team workspace | `name: String`, `owner_id: String`, `state: State<CloudState>` |
| `get_audit_logs` | Retrieve audit entries | `limit: Option<usize>`, `state: State<CloudState>` |

### Platform Commands

| Command | Description | Parameters |
|---------|-------------|------------|
| `check_accessibility` | Check platform permissions | None |
| `request_accessibility` | Prompt for permission dialog | None |
| `inspect_element` | Get UI element info at coords | `x: i32`, `y: i32`, `engine: State<GhostEngine>` |
