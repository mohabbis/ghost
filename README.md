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
