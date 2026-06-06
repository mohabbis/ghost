# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What Ghost is

Ghost is a macOS-first AI widget that **watches what the user does on screen, then helps them automate it**. The core loop: observe user actions (clicks, UI elements) → understand intent → replay / automate those actions. The current scaffold implements the primitive observe-and-replay layer; the AI reasoning layer on top is the project's direction.

This is an early-stage, pre-product codebase (single `init: tauri scaffold` commit). Expect to build out, not just maintain.

## Architecture

Tauri 2 desktop app. Two halves talk over Tauri's IPC bridge:

- **Frontend** (`src/`): plain vanilla HTML/CSS/JS — **no bundler, no npm, no `package.json`**. `tauri.conf.json` sets `frontendDist` to `../src`, so files are served as-is. `withGlobalTauri: true` exposes the API on `window.__TAURI__` (e.g. `window.__TAURI__.core.invoke`, and the event API for listening). There is no dev server; the frontend is static.
- **Backend** (`src-tauri/`): Rust. `lib.rs` registers the Tauri command handlers; `commands.rs` is the thin `#[tauri::command]` IPC surface; `macos_ax.rs` holds the actual macOS system integration.

The observe/replay engine lives in `macos_ax.rs` and is **macOS-only by design** — every command in `commands.rs` is `#[cfg(target_os = "macos")]` gated and returns `Err("Mac only")` (or `true`) on other platforms. The three system capabilities:

- **Accessibility permission** (`AXIsProcessTrustedWithOptions`) — `check_permissions` (no prompt) vs `request_permissions` (prompts the user). The app is useless until granted; gate UX on `check_accessibility`.
- **Recording** (`start_event_tap` / `stop_event_tap`) — a `CGEventTap` listens (read-only) for `LeftMouseDown` globally and emits a `ghost:click-captured` event to the frontend with `(x, y)`. The tap runs `CFRunLoop::run_current()` on a spawned thread. `get_element_at_point` can resolve the AX element (title, role) under a coordinate — the hook for capturing *what* was clicked, not just where.
- **Replay** (`click_at`) — uses `enigo` to move the mouse and synthesize a left click at a coordinate.

State (`EVENT_TAP`, `APP_HANDLE`) is held in `static` globals in `macos_ax.rs`. `APP_HANDLE` is a `static mut` accessed in `unsafe` blocks — known footgun; revisit if recording is started/stopped concurrently.

### IPC contract (Rust ↔ JS)

Commands registered in `lib.rs::run()` via `generate_handler!`: `check_accessibility`, `request_accessibility`, `start_recording`, `stop_recording`, `replay_click`. Call from JS with `invoke("start_recording")` etc. Recording results come back asynchronously as the `ghost:click-captured` **event**, not as a return value — listen with the Tauri event API.

> Note: `src/main.js` is still the Tauri template (`greet`/`#greet-form`) and does **not** match the real command surface above. Replacing it with a real Ghost UI that drives the accessibility/record/replay commands is expected early work.

## Commands

Run from the repo root.

- **Run the app (dev):** `cargo tauri dev` (from `src-tauri/`, or `cargo tauri dev` with the CLI installed). No `npm run tauri` — there is no Node toolchain here.
- **Build the backend:** `cd src-tauri && cargo build` (release: `cargo build --release`).
- **Bundle the app:** `cargo tauri build` → produces the `.app`/`.dmg` (bundle targets `all`).
- **Check / lint Rust:** `cd src-tauri && cargo check` and `cargo clippy`.
- **Tests:** none yet. Rust tests would run via `cd src-tauri && cargo test`.

There is no frontend build/lint/test step because the frontend is static vanilla JS.

## Gotchas

- Granting Accessibility permission applies to the **running binary**. In `cargo tauri dev` the dev binary path changes across rebuilds, so macOS may re-prompt / require re-granting in System Settings → Privacy & Security → Accessibility. A stable `.app` from `cargo tauri build` is more reliable for testing real recording.
- `bundle identifier` is `com.muhammadrafiq.ghost`; macOS ties the permission grant to it.
- `src-tauri/target/` and `src-tauri/gen/` are build artifacts. `Cargo.lock` IS committed (binary crate).
