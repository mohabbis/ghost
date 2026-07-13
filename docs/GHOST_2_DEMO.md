# Ghost 2.0 Demo Workflow

Ghost 2.0 converges Organizer, Routines, and MCP onto one **Action Plan** runtime:

```text
Capture → Review → Approve → Execute → Verify → Recover
```

## Invoice → Finance workflow (5-minute demo)

### Setup

1. Create a Zone pointing at your **Downloads** folder (read + create + move).
2. Place a sample file such as `invoice_jan.pdf` in Downloads.

### Run the demo plan (IPC)

```text
action_plan_demo(downloads, finance_root)
  → review semantic steps in UI
execute_action_plan(source: demo, downloads, finance_root)
  → receipt + undo
```

### Expected semantic steps

1. Create Finance/Invoices folder
2. Rename invoice (dated prefix)
3. Move to Finance/Invoices
4. Open TextEdit
5. Insert log entry
6. Save document (Cmd+S)
7. Verify file exists in Finance folder

### Verification

Each step records **Expected / Observed / Verified** in the execution receipt.

### Recovery

Filesystem steps write undo data before mutation. Use **Undo this run** or `undo_action_plan_execution`.

## Unified entry points

| Source | Compile command | Execute command |
|---|---|---|
| Organizer | `action_plan_from_zone` | `execute_action_plan` (organizer source) |
| Routine | `action_plan_from_events` | `replay_workflow` / `execute_routine_action_plan` |
| MCP | (zone plan hash token) | `mcp execute_approved_plan` → same runtime via `execute_zone` |
| Demo | `action_plan_demo` | `execute_action_plan` (demo source) |

## macOS semantic helper (optional)

Build on macOS:

```bash
swiftc -o ghost-ax-helper native/macos/GhostAXHelper.swift
```

JSON-line protocol: `{"op":"frontmost_app"}`, `{"op":"permission_status"}`.

Rust platform code remains authoritative; the helper is an optional upgrade path.

## Tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml ghost2_pipeline
cargo test --manifest-path src-tauri/Cargo.toml runtime::
cargo test --manifest-path src-tauri/Cargo.toml action_plan::
```
