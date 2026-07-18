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
5. Focus document (Accessibility)
6. Insert log entry (semantic `set_value`, not coordinate typing)
7. Save document (Cmd+S)
8. Verify file exists in Finance folder

### Verification

Each step records **Expected / Observed / Verified** in the execution receipt.
After closing the completion modal, reopen **Replay History → Verification
receipts** to inspect the persisted result and its per-step verification chips.

### Approval moment

Nothing runs until you approve. The review modal trust strip and Confirm dialog
both say so explicitly — Confirm & Replay / Review again / Cancel — before Ghost
issues the one-shot approval token.

### Recovery

Filesystem steps write undo data before mutation. Use **Undo this run** or `undo_action_plan_execution`.

## Unified entry points

| Source | Compile command | Execute command |
|---|---|---|
| Organizer | `action_plan_from_zone` | `execute_action_plan` (organizer source) |
| Routine | `action_plan_from_events` | `replay_workflow` / `execute_routine_action_plan` |
| MCP | (zone plan hash token) | `ghost.execute_approved_plan` → `run_persisted_action_plan` (`PlanSource::Mcp`) |
| Demo | `action_plan_demo` | `execute_action_plan` (demo source) |

## macOS semantic helper

Ghost ships **GhostAXHelper** inside the macOS app bundle
(`Ghost.app/Contents/MacOS/ghost-ax-helper`). Release CI builds it automatically;
no `GHOST_AX_HELPER` env var is required for installed builds.

Local development on macOS:

```bash
make ax-helper
# optional override:
export GHOST_AX_HELPER="$(pwd)/native/macos/ghost-ax-helper"
```

Operations: `resolve_target`, `list_matches`, `activate_element`, `set_value`, `verify_element`, `enumerate_children`, `permission_status`, `frontmost_app`.

Request fields (optional beyond `op` / `app` / `role`): `title`, `identifier`,
`bundle_id`, `window_title`, `fingerprint`, `value`, `expected_value`, `path`
(for `capture_still`).
Responses may include `ax_quality` (0–100), `identifier`, `actionable`, and for
capture ops `path` / `width` / `height`.

Capture ops (Screen Recording; Accessibility not required):
`capture_permission_status`, `capture_still`.

Precondition ops (no Accessibility required): `app_running`.

Ambiguous matches are refused. Stale target fingerprints are rejected at execution time.
`focus_target` / `set_target_value` check `app_running`, prefer AX, then OCR vision
fallback when `title` is set. Postconditions use AX verify or OCR text presence.
`list_matches` is the read-only exception: it enumerates every candidate element (fingerprint + value + quality) so an ambiguous target can be inspected and disambiguated before an action is approved — it mutates nothing.

Targeting prefers exact bundle id / app name, then contains-fallback; `window_title`
narrows the AX search root; activate tries `AXPress` before raise/focus. When AX
fails or scores insufficient and `title` is set, `focus_target` may fall back to
ScreenCaptureKit still + Vision OCR + coordinate click. See
[`macos-automation-architecture.md`](macos-automation-architecture.md).

### Real Mac validation (required before claiming demo-complete)

1. Grant **Accessibility** to Ghost (System Settings → Privacy & Security).
2. Run the invoice demo end-to-end.
3. Confirm TextEdit receives the log via AX `set_value` (not enigo fallback).
4. Record a screen walkthrough for the release PR.

Rust platform code remains authoritative when the helper is absent; UI steps fall back to keyboard replay where safe.

## Tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml ghost2_pipeline
cargo test --manifest-path src-tauri/Cargo.toml ghost2_runtime_completion
cargo test --manifest-path src-tauri/Cargo.toml runtime::
cargo test --manifest-path src-tauri/Cargo.toml action_plan::
```
