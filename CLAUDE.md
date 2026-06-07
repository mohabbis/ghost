# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What Ghost is

Ghost is a cross-platform (macOS + Windows) AI automation desktop app that **watches what the user does on screen, then helps them automate it**. The core loop: observe user actions (clicks, keystrokes, UI elements) → understand intent → replay / automate those actions with AI-assisted analysis.

Five phases are fully implemented: Foundation (record/replay), Workflow Management, AI Analysis, Advanced Reliability, and Cloud Sync & Enterprise features. A Smart Observer Mode (proactive pattern learning) is also in place.

## Architecture

Tauri 2 desktop app. Two halves talk over Tauri's IPC bridge:

- **Frontend** (`src/`): plain vanilla HTML/CSS/JS — **no bundler, no npm, no `package.json`**. `tauri.conf.json` sets `frontendDist` to `../src`, so files are served as-is. `withGlobalTauri: true` exposes the Tauri API on `window.__TAURI__`. There is no dev server; the frontend is static.
- **Backend** (`src-tauri/`): Rust. `lib.rs` registers all 69 Tauri command handlers. The real logic lives in `engine.rs` (GhostEngine orchestrator) and the `core/` + `platform/` module trees.

### Backend module tree

```
src-tauri/src/
├── main.rs              # entry point; calls ghost_lib::run()
├── lib.rs               # Tauri app builder; registers all 69 commands via generate_handler!
├── commands.rs          # thin #[tauri::command] IPC surface (~580 lines)
├── engine.rs            # GhostEngine — orchestrates recording, replay, workflow mgmt (~867 lines)
├── core/
│   ├── mod.rs           # re-exports
│   ├── events.rs        # InputEvent enum, ElementInfo, Workflow, WorkflowMetadata structs
│   ├── traits.rs        # InputRecorder, ElementLocator, ReplayEngine traits (platform-agnostic)
│   ├── ai.rs            # WorkflowAnalyzer: pattern detection, optimization suggestions, naming
│   ├── llm.rs           # LLMProvider trait; OpenAI, Claude, Local fallback implementations
│   ├── cloud.rs         # CloudSyncManager, Workspace, AuditLog, MemberRole (RBAC)
│   ├── execution.rs     # ExecutionRecord, ExecutionHistory, ExecutionStatus
│   ├── knowledge.rs     # KnowledgeBase for Smart Observer Mode; LearnedPattern, ProactiveSuggestion
│   ├── vision.rs        # SSIM image comparison, screenshot capture
│   ├── wait.rs          # Smart wait conditions (ElementVisible, TextPresent, ImageMatches, etc.)
│   └── security.rs      # (stub; referenced in mod.rs)
└── platform/
    ├── mod.rs           # re-exports platform-specific implementations
    ├── macos.rs         # CGEventTap recording, AXUIElement inspection, enigo replay
    └── windows.rs       # Win32 hooks, UIA (UI Automation), enigo replay
```

### Frontend files

```
src/
├── index.html   # full recording/replay/cloud UI (not a marketing page)
├── main.js      # ~1000 lines; all Tauri IPC logic, recording controls, workflow mgmt, cloud sync, Observer mode
└── styles.css   # dark theme design system (purple/orange palette)
```

## IPC contract (Rust ↔ JS)

### Tauri events (async, backend → frontend)

| Event | Payload | When emitted |
|---|---|---|
| `ghost:event` | `InputEvent` (JSON) | Each captured input during recording |

Listen from JS:
```javascript
const { listen } = window.__TAURI__.event;
await listen("ghost:event", (event) => {
  recordedEvents.push(event.payload);
});
```

### Commands (69 total, registered in `lib.rs`)

Call from JS with `window.__TAURI__.core.invoke("command_name", { ...args })`.

**Accessibility**
- `check_accessibility` → `bool`
- `request_accessibility` → `bool`

**Recording & Playback**
- `start_recording`
- `stop_recording`
- `get_recorded_events` → `Vec<InputEvent>`
- `replay_workflow(events, speed?)` → streams progress via `ghost:event`
- `cancel_replay`
- `pause_replay`
- `resume_replay`
- `is_replay_paused` → `bool`
- `is_replay_running` → `bool`
- `set_playback_speed(speed: f64)`
- `get_playback_speed` → `f64`
- `inspect_element(x, y)` → `ElementInfo`

**Workflow Management**
- `save_workflow(name, events)`
- `load_workflow(name)` → `Workflow`
- `delete_workflow(name)`
- `list_workflows` → `Vec<String>`
- `save_workflow_with_metadata(workflow)`
- `load_workflow_with_metadata(name)` → `Workflow`
- `save_workflow_with_sidecar(workflow)` — saves JSON + human-readable sidecar
- `generate_workflow_from_prompt(prompt, screenshot?)` — LLM-driven workflow creation

**AI Analysis**
- `analyze_workflow(events)` → `WorkflowAnalysis` (patterns, optimizations, reliability score)
- `optimize_workflow(events)` → `Vec<InputEvent>` (optimized sequence)
- `suggest_workflow_name(events)` → `String`
- `analyze_and_tag_workflow(events)` → `Vec<InputEvent>` (events with SemanticTag)

**Advanced Reliability (Phase 4)**
- `replay_with_reliability(workflow, retry_config)` — exponential backoff, checkpoints, element validation

**Cloud Sync (Phase 5)**
- `init_cloud_sync`
- `cloud_authenticate(token)`
- `cloud_sync_workflows`
- `create_workspace(name, description)`
- `get_audit_logs` → `Vec<AuditLog>`

**Execution & Analytics**
- `get_execution_history(workflow_name)` → `Vec<ExecutionRecord>`
- `get_all_executions` → `Vec<ExecutionRecord>`
- `get_workflow_analytics(workflow_name)` → analytics summary

**Visual Regression**
- `capture_baseline_screenshot(workflow_name, step_index)`
- `replay_with_visual_check(workflow, threshold?)` — SSIM-based comparison

**Data Sources**
- `create_data_source(type, config)` — CSV, JSON, or environment variable sets
- `load_variables(source_name)` → `HashMap<String, String>`

**Smart Observer Mode**
- `start_observer`
- `stop_observer`
- `is_observer_active` → `bool`
- `set_observer_interval(ms)`
- `observe_events` → snapshot of recent activity
- `get_proactive_suggestions` → `Vec<ProactiveSuggestion>`
- `get_learned_patterns` → `Vec<LearnedPattern>`
- `get_app_usage_stats` → `Vec<AppUsageStat>`
- `generate_geek_insights` → `GeekDetails`

## Key data types

### InputEvent (core/events.rs)

```rust
enum InputEvent {
    MouseClick { x, y, button, element: Option<ElementInfo>, timestamp, retry_count, semantic_tag },
    Key { code, chars, modifiers, action, timestamp, retry_count, semantic_tag },
    Scroll { dx, dy, phase, timestamp },
    Delay { ms, timestamp },
    Wait { condition: WaitCondition, timeout_ms, poll_interval_ms },
    VisualCheck { baseline_screenshot, threshold, on_mismatch },
    Variable { name, value_template, var_type },
    VariableRef { name },
}
```

### Workflow

```rust
struct Workflow {
    name: String,
    events: Vec<InputEvent>,
    metadata: WorkflowMetadata,
    reliability: Option<ReliabilitySettings>,
}

struct WorkflowMetadata {
    name, description, tags: Vec<String>,
    created_at, updated_at,
    estimated_duration_ms: u64,
    reliability_score: f32,   // 0.0–1.0
    element_confidence: f32,  // 0.0–1.0
}
```

### ReliabilitySettings (Phase 4)

```rust
struct ReliabilitySettings {
    retry_config: RetryConfig,      // max_attempts, backoff_ms, backoff_multiplier
    checkpoints: Vec<usize>,        // step indices requiring confirmation
    validate_elements: bool,
    continue_on_error: bool,
}
```

## Platform-specific implementations

**macOS (`platform/macos.rs`):**
- Recording: `CGEventTap` session tap (read-only), catches `LeftMouseDown/Up`, key events, scroll
- Element lookup: `AXUIElement` system-wide API; extracts role, title, value, app name
- Replay: `enigo` for mouse movement and click synthesis
- Accessibility permission: `AXIsProcessTrustedWithOptions`

**Windows (`platform/windows.rs`):**
- Recording: Win32 `SetWindowsHookEx` (WM_MOUSE, WM_KEYBOARD hooks)
- Element lookup: UIA (UI Automation) API; finds elements by HWND and point
- Replay: `enigo` cross-platform

## LLM integration (core/llm.rs)

Trait-based abstraction. Activated by `generate_workflow_from_prompt` and `analyze_and_tag_workflow`.

- Providers: OpenAI, Claude, Local (fallback heuristics — no API key required)
- Configuration: `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` environment variables
- Input: prompt string + optional screenshot (PNG bytes) + element context
- Output: `Vec<InputEvent>`

The LLM instance is a process-wide singleton via `OnceLock`.

## Commands (dev workflow)

Run from the repo root. **There is no Node toolchain — no npm, no package.json.**

```bash
# Run in dev mode
cargo tauri dev

# Build Rust backend only
cd src-tauri && cargo build

# Release build
cd src-tauri && cargo build --release

# Bundle .app / .dmg (macOS) or .exe / .msi (Windows)
cargo tauri build

# Lint
cd src-tauri && cargo check
cd src-tauri && cargo clippy

# Tests (none yet; add with #[cfg(test)] in any module)
cd src-tauri && cargo test
```

There is no frontend build or lint step — the frontend is static vanilla JS.

## Releases & macOS code signing

Releases are tag-driven. Pushing a `v*` tag fires `.github/workflows/release.yml`,
which builds `Ghost.dmg` (universal macOS) and `Ghost_Setup.exe` (Windows) and
attaches them to a GitHub Release. See `RELEASING.md` for the tagging steps.

macOS signing is handled by the `Configure macOS signing` step in that workflow:

- If the `APPLE_CERTIFICATE` secret is **absent**, the app is **ad-hoc signed**
  (`APPLE_SIGNING_IDENTITY=-`). It runs locally but downloaded copies are still
  quarantined, so Gatekeeper shows "Apple could not verify…" on first launch.
- If the Apple Developer secrets are **present** (`APPLE_CERTIFICATE`,
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
  `APPLE_PASSWORD`, `APPLE_TEAM_ID`), the Tauri bundler signs with the Developer
  ID **and notarizes**, so the app opens with no prompt. No YAML change is needed
  to switch modes — the step auto-detects the secret and exports the env vars the
  bundler reads.

User-side workaround for an ad-hoc/unsigned download:
`xattr -dr com.apple.quarantine /Applications/ghost.app`.

Only notarization (paid Apple Developer ID) removes the Gatekeeper dialog —
there is no free bypass.

## Data persistence

Workflows and baselines are stored in the platform's data directory:

```
tauri::api::path::data_dir() / "ghost" / <workflow_name>.json
```

Sidecar files (human-readable) use `.sidecar.txt` suffix.

## State management patterns

- **GhostEngine**: managed as `tauri::State<Mutex<GhostEngine>>` — always lock before use
- **CloudState**: separate `tauri::State<Mutex<CloudState>>` for cloud sync state
- **Cancellation**: `Arc<AtomicBool>` stop flag checked in the replay loop
- **Recording bridge**: `mpsc` channel from the platform recorder thread → Tauri event emitter thread
- **LLM singleton**: `OnceLock<Box<dyn LLMProvider>>` initialized once at first use

## Gotchas

- **Accessibility permission** applies to the running binary path. `cargo tauri dev` changes the binary path on each rebuild, so macOS may re-prompt. A stable `.app` from `cargo tauri build` is more reliable for testing real recording.
- **Bundle identifier** is `com.muhammadrafiq.ghost`; macOS ties the permission grant to it.
- **`src-tauri/target/` and `src-tauri/gen/`** are build artifacts — never edit or commit them. `Cargo.lock` IS committed (binary crate convention).
- **Windows screenshot capture** in `vision.rs` is a stub — SSIM visual regression is macOS-only for now.
- **Cloud sync** (`cloud.rs`) stores data in-memory only — no real remote API calls are wired yet.
- **`APP_HANDLE` static mut** in `platform/macos.rs` is accessed with `unsafe` — known footgun; avoid concurrent `start_recording`/`stop_recording` calls.

## Frontend conventions

- No framework, no build step. DOM manipulation via `document.querySelector` and `addEventListener`.
- All Tauri calls go through `window.__TAURI__.core.invoke(...)` and `window.__TAURI__.event.listen(...)`.
- Global JS state: `isRecording`, `recordedEvents[]`, `isPlaying`, `isPaused`, `playbackSpeed`.
- UI is organized into collapsible sections in `index.html`: Recording, Workflow Management, AI Analysis, Cloud Sync, Smart Observer, Phase 4 (visual/data), Event Timeline.
- Modals `#analysis-modal` and `#audit-modal` display workflow analysis and audit log results.
- CSS design tokens: accent purple `#8d7bff`, warm orange `#ffb86b`, success mint `#83f6c4`, dark bg `#070813`.

## Key dependencies (Cargo.toml)

| Crate | Purpose |
|---|---|
| `tauri 2` | Desktop app framework |
| `enigo` | Cross-platform mouse/keyboard synthesis |
| `tokio` (full) | Async runtime |
| `serde`, `serde_json` | Serialization for IPC |
| `core-foundation`, `core-graphics`, `accessibility-sys` | macOS system APIs |
| `image 0.24`, `ssim 0.1` | Visual regression screenshots |
| `reqwest` | HTTP client for cloud sync / LLM APIs |
| `uuid` | Workflow and execution IDs |
| `anyhow` | Error handling |
| `regex` | Pattern detection in AI module |
| `tracing`, `tracing-subscriber`, `tracing-chrome` | Structured logging / profiling |
