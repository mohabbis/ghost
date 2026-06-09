# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What Ghost is

Ghost is a cross-platform (macOS + Windows) AI automation desktop app that **watches what the user does on screen, then helps them automate it**. The core loop: observe user actions (clicks, keystrokes, UI elements) → understand intent → replay / automate those actions with AI-assisted analysis.

Five phases are fully implemented: Foundation (record/replay), Workflow Management, AI Analysis, Advanced Reliability, and Cloud Sync & Enterprise features. A Smart Observer Mode (proactive pattern learning) is also in place.

## Architecture

Tauri 2 desktop app. Two halves talk over Tauri's IPC bridge:

- **Frontend** (`src/`): plain vanilla HTML/CSS/JS — **no bundler, no npm, no `package.json`**. `tauri.conf.json` sets `frontendDist` to `../src`, so files are served as-is. `withGlobalTauri: true` exposes the Tauri API on `window.__TAURI__`. There is no dev server; the frontend is static.
- **Backend** (`src-tauri/`): Rust. `lib.rs` registers all 48 Tauri command handlers. The real logic lives in `engine.rs` (GhostEngine orchestrator) and the `core/` + `platform/` module trees.

### Backend module tree

```
src-tauri/src/
├── main.rs              # entry point; calls ghost_lib::run()
├── lib.rs               # Tauri app builder; registers all 54 commands via generate_handler!
├── commands.rs          # thin #[tauri::command] IPC surface (~640 lines)
├── engine.rs            # GhostEngine — orchestrates recording, replay, workflow mgmt (~975 lines)
├── config.rs            # GhostConfig — general/recording/replay/AI/privacy/performance settings + validation
├── error.rs             # GhostError/ErrorKind — structured, user-friendly error type with suggestions
├── performance.rs       # PerformanceMonitor — operation timers and metrics collection
├── telemetry.rs         # TelemetryManager — opt-in usage events and UsageStats
├── core/
│   ├── mod.rs           # re-exports
│   ├── events.rs        # InputEvent enum, ElementInfo, Workflow, WorkflowMetadata structs
│   ├── traits.rs        # InputRecorder, ElementLocator, ReplayEngine traits (platform-agnostic)
│   ├── ai.rs            # WorkflowAnalyzer: pattern detection, optimization suggestions, naming
│   ├── llm.rs           # LLMProvider trait; OpenAI, Claude, Local fallback implementations
│   ├── cloud.rs         # CloudSyncManager, Workspace, AuditLog, MemberRole (RBAC)
│   ├── execution.rs     # ExecutionRecord, ExecutionHistory, ExecutionStatus
│   ├── knowledge.rs     # KnowledgeBase for Smart Observer Mode; LearnedPattern, ProactiveSuggestion
│   ├── vision.rs        # SSIM image comparison, cross-platform screenshot capture
│   ├── wait.rs          # Smart wait conditions (ElementVisible, TextPresent, ImageMatches, etc.)
│   └── security.rs      # path sanitization, input validation, rate limiting (audit submodule is a stub)
└── platform/
    ├── mod.rs           # re-exports platform-specific implementations
    ├── macos.rs         # CGEventTap recording, AXUIElement inspection, enigo replay
    └── windows.rs       # Win32 hooks, UIA (UI Automation), enigo replay
```

> **Note:** `config.rs`, `performance.rs`, and `telemetry.rs` are now wired into the live path:
> `GhostEngine` constructs `GhostConfig` (drives playback speed + LLM provider), a
> `TelemetryManager` (gated by `config.privacy.telemetry_enabled`), and a `PerformanceMonitor`
> (gated by `config.performance.profiling_enabled`). Recording/replay call into telemetry +
> perf, and `get_telemetry_stats`/`export_telemetry`/`get_performance_summary` expose them over
> IPC. `core::security`'s `sanitize_workflow_path` + `validate_prompt` are also called from
> `commands.rs`. Still standalone: **`error.rs`** (`GhostError`/`ErrorKind` are not yet returned
> from commands — they still hand back `String`/`anyhow`), plus `core::security`'s `SimpleCrypto`,
> `rate_limit`, `validate_screenshot/csv/coordinates`, and the `audit` submodule stub. Grep for
> `use crate::error` before assuming structured errors reach command results.

### Frontend files

```
src/
├── index.html   # standalone app shell (~125 lines): header, accessibility-permission banner, Recording/Workflows/Observer/Cloud panels, analysis + audit modals
├── main.js      # ~950 lines; all Tauri IPC logic, recording controls, workflow mgmt, cloud sync, Observer mode
├── app.css      # app-shell-only layout (.app-shell/.app-grid/.panel/.banner…), built on the CSS custom properties from styles.css
└── styles.css   # dark theme design system / design tokens (purple/orange palette), shared with the marketing site
```

> **Important — `src/` is the APP, `public/` is the marketing SITE; they are NOT interchangeable.**
> They share `styles.css` and asset files, but `src/index.html` + `src/main.js` are a desktop app
> shell wired to Tauri IPC, while `public/index.html` + `public/main.js` are the marketing landing
> page. Earlier these had drifted into byte-identical copies of the marketing site, which made the
> shipped app render the website instead of the app — don't re-sync them blindly. `app.css` exists
> only under `src/` (the marketing site has no app shell). When changing shared pieces (`styles.css`,
> tokens, assets) keep both in sync by hand; when changing app vs. site behavior, edit only the
> relevant tree.

- Buttons are wired exclusively via `addEventListener` in a `wireUpControls()` pass on
  `DOMContentLoaded` — **never** inline `onclick="fn()"`, because `<script type="module">` does not
  expose top-level functions on `window`, so inline handlers throw `ReferenceError`. Dynamically
  injected markup (modal buttons, suggestion cards) uses `data-*` attributes + event delegation on
  `document.body` for the same reason.

### Marketing website (`public/`)

`public/` is the static marketing site at ghost.muharafiq.com — **not** served by the Tauri app.
It mirrors `src/`'s shared files (`styles.css`, `assets/`, `downloads/`, favicons) but has its own
marketing `index.html`/`main.js` (see the warning above). Download links on the site point at the
latest GitHub Release assets (`Ghost.dmg`, `Ghost_Setup.exe`), not the files checked into
`public/downloads/`. See `DEPLOYMENT.md`.

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

### Commands (54 total, registered in `lib.rs`)

Call from JS with `window.__TAURI__.core.invoke("command_name", { ...args })`.

**Accessibility**
- `check_accessibility` → `bool`
- `request_accessibility` → `bool`

**Config & Observability**
- `get_config` → `GhostConfig`
- `update_config(config)` — validates, persists, and live-applies (speed, LLM, telemetry/perf toggles)
- `get_telemetry_stats` → `UsageStats` (empty unless `privacy.telemetry_enabled`)
- `export_telemetry` → `String` (JSON: session id + stats + events)
- `get_performance_summary` → `PerformanceSummary` (empty unless `performance.profiling_enabled`)

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
- Accessibility permission: `check_accessibility` calls `AXIsProcessTrusted()` (no prompt); `request_accessibility` calls `AXIsProcessTrustedWithOptions` with `kAXTrustedCheckOptionPrompt` to surface the system dialog. Both come from the `accessibility-sys` crate; the option dictionary is built with `core-foundation` safe wrappers.
- Recording run loop: the event-tap thread adds its `CFRunLoopSource` to the current run loop under `kCFRunLoopCommonModes`, which is the real CoreFoundation symbol pulled in via `extern "C" { static kCFRunLoopCommonModes: CFStringRef; }` — **not** a `std::ptr::null()` placeholder. Passing null here crashes the recorder thread (`EXC_BREAKPOINT` inside `CFRunLoopAddSource`→`CFHash`) within seconds of `start_recording`.

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

# Tests: unit tests live inline (#[cfg(test)]) in config.rs, error.rs,
# performance.rs, telemetry.rs; integration tests in src-tauri/tests/integration_test.rs
# (config, error handling, events, workflow ops) and e2e in src-tauri/tests/e2e.rs
cd src-tauri && cargo test

# Run a single test by name (substring match)
cd src-tauri && cargo test test_name_substring
```

There is no frontend build or lint step — the frontend is static vanilla JS.

## CI/CD (`.github/workflows/`)

- **`rust.yml`** — runs on push/PR to `main`/`master`/`develop`: `cargo check`, `cargo test`,
  `cargo clippy`, `cargo fmt --check`, plus per-platform release builds. Ubuntu jobs need
  `libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf` installed
  before `cargo` runs (see `CI_FIX_SUMMARY.md` for the history of fixes here — warnings are
  intentionally allowed, not denied, in check/test/clippy).
- **`release.yml`** — tag-driven (`v*`), builds and signs the desktop bundles (see below).
- **`deploy-website.yml`** — triggered by changes under `public/**`; deploys the marketing
  site to Vercel and validates download links / structured data in `public/index.html`.
- **`deny.toml`** at the repo root configures `cargo-deny` for license and advisory checks
  (MIT/Apache-2.0/BSD/ISC allow-listed; (A)GPL denied).

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
- **Windows screenshot capture** in `vision.rs` shells out to PowerShell (`System.Drawing`/`System.Windows.Forms`) to a temp PNG; macOS uses the `screencapture` CLI. Both feed the same SSIM comparison in `replay_with_visual_check`.
- **Cloud sync** (`cloud.rs`) stores data in-memory only — no real remote API calls are wired yet (`cloud_authenticate`/`cloud_sync_workflows` don't touch `reqwest`).
- **`error.rs` is not yet integrated** — it compiles, has its own tests, and is exported from `lib.rs`, but `engine.rs`/`commands.rs` still return `String`/`anyhow` rather than `GhostError`. (`config.rs`/`performance.rs`/`telemetry.rs` ARE now wired — see the note under the backend module tree above.) Don't assume `GhostError` shows up in command results just because the type exists.
- **`src/` and `public/` must stay in sync by hand** — `public/` is a parallel copy of the frontend deployed as the marketing site (see `DEPLOYMENT.md`); editing one without the other causes drift between the app UI and the website.
- **macOS recording state** in `platform/macos.rs` lives behind `Arc<Mutex<Option<TapState>>>` (with manual `unsafe impl Send/Sync` on the tap types) rather than a global static — still avoid overlapping `start_recording`/`stop_recording` calls since the `CGEventTap` lifecycle is stateful.

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
| `tauri 2`, `tauri-plugin-opener` | Desktop app framework + opener plugin |
| `enigo` | Cross-platform mouse/keyboard synthesis |
| `tokio` (full) | Async runtime |
| `serde`, `serde_json` | Serialization for IPC |
| `core-foundation`, `core-graphics`, `accessibility-sys` | macOS system APIs (macOS-only target deps) |
| `image 0.24`, `ssim 0.1`, `rusttype` | Visual regression screenshots and image annotation |
| `reqwest` (json) | HTTP client for cloud sync / LLM APIs |
| `uuid` (v4) | Workflow and execution IDs |
| `anyhow` | Error handling |
| `regex` | Pattern detection in AI module, workflow-name validation |
| `tracing`, `tracing-subscriber`, `tracing-chrome` (optional, `profiling` feature) | Structured logging / profiling |
| `threadpool` | Background task execution |
| `base64` | Encoding screenshots / binary payloads for IPC |
| `async-trait` | Async methods on the `LLMProvider` trait |
| `dirs` | Cross-platform data/config directory resolution |

`profiling` is the only Cargo feature (enables `tracing-chrome`); `default = []`.
