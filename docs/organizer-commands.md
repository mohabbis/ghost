# Ghost Organizer — command bridge

The Organizer backend (planner → executor → audit → undo) is exposed to the UI by
`src-tauri/src/commands/organizer.rs`. This is the IPC contract the frontend
(`src/main.js`) calls. It surfaces the trust pipeline in order:

```text
Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
```

See `docs/organizer-planner.md` and `docs/organizer-executor.md` for the backend
logic, and `docs/command-registry.md` for the risk inventory.

## Trust property

`organizer_execute` does **not** accept a plan from the frontend. It re-plans
server-side from the Zone id and the executor independently re-checks every
action through `policy::evaluate`. The preview the UI shows (`organizer_plan`) and
the plan actually executed come from the same deterministic backend against the
same persisted rules, so a stale or tampered plan posted from JS can never reach
the filesystem.

## Commands

| Command | Args | Returns | Effect |
| --- | --- | --- | --- |
| `organizer_list_zones` | — | `Zone[]` | Read Zones. |
| `organizer_list_folder_rules` | `{ zoneId }` | `FolderRule[]` | Read a Zone's approved folders. |
| `organizer_create_zone` | `{ name, description? }` | `Zone` | Create a Zone (defaults to `Ask`). |
| `organizer_add_folder_rule` | `{ zoneId, rule: FolderRule }` | — | Persist a boundary. Rejects `can_delete: true`. |
| `organizer_plan` | `{ zoneId }` | `OrganizerPlan` | **Read-only** preview; mutates nothing. |
| `organizer_execute` | `{ zoneId }` | `ExecutionResult` | Apply the approved plan; audited + undoable. |
| `organizer_list_executions` | — | `ExecutionSummary[]` | History, newest first. |
| `organizer_undo` | `{ executionId }` | `UndoReport` | Reverse a past run. |

`ExecutionResult = { execution_id, report }` where `report` is the
`ExecutionReport` (`applied`, `skipped`, `failed`, `audit`, `undo`).

## JSON shapes (serde wire format)

- **Capability** — internally tagged on `kind` (snake_case):
  `{ "kind": "move_file", "from": "...", "to": "..." }`,
  `{ "kind": "create_folder", "path": "..." }`,
  `{ "kind": "rename_file", "from": "...", "to": "..." }`.
- **PolicyDecision** — tagged on `decision`:
  `{ "decision": "allow" }`,
  `{ "decision": "deny", "reason": "..." }`,
  `{ "decision": "require_confirmation", "reason": "...", "risk": "medium" }`.
- **ActionOutcome** (inside each audit event) — tagged on `outcome`:
  `{ "outcome": "applied" }`,
  `{ "outcome": "skipped", "reason": "..." }`,
  `{ "outcome": "failed", "error": "..." }`.
- **AuditLog** serializes transparently as an array of
  `{ capability, outcome, at }` events.
- **UndoReport** = `{ reverted, skipped, failed }`.

## Registration

Commands are re-exported from `src-tauri/src/commands.rs` (`mod organizer;
pub use organizer::*;`) and listed in the `generate_handler!` macro in
`src-tauri/src/lib.rs`.
