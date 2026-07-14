# Ghost Native macOS Preview

This directory contains the platform-native SwiftUI shell for Ghost. It does not
replace or fork the trusted Rust implementation.

```text
SwiftUI / AppKit adapters
          |
          | versioned JSON-lines commands and events
          v
ghost_core_bridge
          |
          v
Existing Ghost Rust planner, policy, executor, audit, undo, storage, and MCP
```

## First vertical slice

The preview implements one complete Organizer workflow:

```text
Choose Folder -> Scan -> Review Plan -> Approve -> Execute -> Receipt -> Undo
```

Swift never performs trusted filesystem mutations. The bridge creates or reuses
a deny-by-default Organizer Zone, generates the plan in Rust, issues a signed
five-minute single-use approval token, replans before execution, writes a
write-ahead execution record, persists the receipt and undo journal, and executes
undo in Rust.

## Build and run

From the repository root on macOS 26 or newer:

```bash
./script/build_and_run.sh
```

Useful modes:

```bash
./script/build_and_run.sh --verify
./script/build_and_run.sh --logs
./script/build_and_run.sh --telemetry
./script/build_and_run.sh --debug
./script/build_and_run.sh --build-only
```

The script builds `ghost_core_bridge`, builds the SwiftPM executable, stages both
inside `dist/Ghost.app`, signs the development bundle, and launches it. The
existing Tauri application remains unchanged and can still be built normally.

## Source layout

```text
Ghost/
  App/              Scenes, commands, and app-wide environment
  Features/         Organizer, History, and Settings surfaces
  RustBridge/       Typed protocol, process client, models, and events
  AppKitBridge/     Narrow native adapters such as NSOpenPanel
  Services/         Permissions and security-scoped bookmark handling
  Views/            Root split view and sidebar
```

## Current bridge choice

Phase 1 uses a bundled local process rather than UniFFI. This keeps the boundary
versioned and testable while reusing the current Rust crate without duplicating
policy or execution logic. Before a hardened sandboxed distribution, evaluate an
XPC service or generated FFI bindings for stronger lifecycle, cancellation, and
sandbox-extension handling.
