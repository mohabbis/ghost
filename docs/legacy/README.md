# Legacy desktop docs

These files describe the **superseded** Rust/Tauri app (`src-tauri/`, `src/`, `apps/macos/`).

They are retained so maintainers can still navigate Organizer / replay / MCP / signing code.
They are **not** the product roadmap. For current work see [`../README.md`](../README.md) and [`../../cloud/`](../../cloud/).

| Doc | Scope |
|---|---|
| `command-registry.md` | Tauri command surface |
| `core` boundaries lived in parent; use code + this shelf | |
| `policy-engine.md` | Deny-by-default policy |
| `organizer-*.md` | Organizer plan / execute / IPC |
| `event-compression.md` / `token-compression.md` | Compression modules |
| `target-resolution.md` | Replay target resolution |
| `GHOST_GUARD.md` | Recording/replay safety |
| `mcp-integration.md` / `mcp-relay.md` / `approval-tokens.md` | MCP surface |
| `filing-profiles.md` | Read-only filing preview |
| `microsoft-auth.md` / `power-bi-integration.md` | Desktop OAuth / Power BI experiment |
| `native-macos-preview.md` | SwiftUI bridge scaffold |
| `*-signing-*.md` / `auto-update.md` / `VERIFY_DOWNLOADS.md` | Release / updater |

Root [`../../RELEASING.md`](../../RELEASING.md) remains the installer release runbook for that app only.
