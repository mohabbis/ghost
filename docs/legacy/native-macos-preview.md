# Native macOS Preview

Status: implementation scaffold for the Ghost 2.0 Organizer vertical slice.

For the longer-term macOS operating layer (AX → Vision → coordinates, ScreenCaptureKit,
permission coordinator, Swift/Rust ownership), see
[`macos-automation-architecture.md`](macos-automation-architecture.md).

## Responsibility boundary

| Layer | Owns | Must not own |
|---|---|---|
| SwiftUI | scenes, navigation, review, approval UI, progress, receipts, history, settings | policy decisions, audit construction, trusted file mutations |
| AppKit adapters | file panels and permission-facing macOS behavior | product rules or execution |
| Rust bridge | stable command/event boundary and process lifecycle | presentation state |
| Existing Rust core | scan, plan, policy, approval validation, mutation, audit, WAL, receipt, undo, storage | SwiftUI rendering |

## Bridge protocol v1

Transport: one JSON object per line over the bundled bridge process's stdin and
stdout. Every message carries `version` and `request_id`.

Commands:

| Command | Risk | Result |
|---|---|---|
| `handshake` | safe-read | protocol/core versions, capabilities, vault state |
| `auth_status` | safe-read | local vault state |
| `auth_unlock` | auth | updated vault state; password stays on the local pipe |
| `auth_lock` | auth | updated vault state |
| `scan` | safe-read | metadata-only scan result and stable `scan_id` |
| `create_plan` | local DB mutation | persisted Zone plus policy-checked plan |
| `approve` | approval | signed, five-minute, single-use token bound to plan hash |
| `execute` | local-mutate | progress events and authoritative execution receipt |
| `receipt` | safe-read | persisted receipt for an execution id |
| `undo` | local-mutate | undo-started event and final undo report |
| `list_executions` | safe-read | local execution history |

## Execution invariants

1. Swift receives stable identifiers, never Rust pointers or raw storage handles.
2. A selected folder becomes one `AskFirst` Zone rule with read/create/move/rename only.
3. Delete and overwrite capabilities are never granted by the native preview.
4. Approval is signed, expires after five minutes, and its nonce is consumed once.
5. Rust replans immediately before execution and compares the canonical plan hash.
6. Policy is evaluated again for every action during execution.
7. The execution row exists before the first filesystem mutation and is updated after each step.
8. Rust persists the final audit receipt and undo journal.
9. Undo refuses to overwrite an occupied origin and removes only empty folders.
10. A configured locked vault blocks Zone creation, approval, execution, and undo.

## Deliberate phase-1 limits

- The bridge is request-serial and does not yet expose cancellation.
- The Swift shell covers Organizer and execution history, not Routines or MCP management UI.
- Security-scoped bookmark persistence is implemented in Swift, but a sandboxed production build should validate extension inheritance or move the bridge to XPC.
- The build script creates a development bundle. Distribution signing, notarization, and App Store sandbox configuration remain release work.
- The native app is additive; the Tauri macOS and Windows interfaces remain operational during migration.
