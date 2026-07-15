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
| `organizer_create_zone` | `{ name, description?, renameDated? }` | `Zone` | Create a Zone (defaults to `Ask`). |
| `organizer_add_folder_rule` | `{ zoneId, rule: FolderRule }` | — | Persist a boundary. Rejects `can_delete: true`. `rule.trust` optional (`automate`/`ask_first`/`never`, default `ask_first`). |
| `organizer_set_rule_trust` | `{ zoneId, path, trust }` | — | Update a rule's trust level in place. Errors if no rule at `path`. |
| `organizer_plan` | `{ zoneId }` | `OrganizerPlan` | **Read-only** preview; mutates nothing. |
| `organizer_execute` | `{ zoneId }` | `ExecutionResult` | Apply the approved plan; audited + undoable. Write-ahead durable (see `docs/organizer-executor.md` "Crash recovery"): a row exists before the first mutation and is updated after every action, not just once at the end. Seals the run into the hash chain and prunes per the retention policy. |
| `organizer_list_executions` | — | `ExecutionSummary[]` | History, newest first; each row has `sealed` and `finished` flags. |
| `organizer_check_unfinished_run` | — | `ExecutionSummary \| null` | A run that began but never finished — almost always a crash mid-run. `null` means the last run ended cleanly. |
| `organizer_dismiss_unfinished_run` | `{ executionId }` | — | Mark an interrupted run resolved without undoing it (the user is fine leaving its changes in place). |
| `organizer_undo` | `{ executionId }` | `UndoReport` | Reverse a past run. Works on unfinished runs too — undoing one also resolves it. |
| `organizer_export_audit` | `{ executionId, format }` | `string` | A past run's audit log as `json` or `csv` text, carrying the run's `hash`/`prev_hash` seal as metadata; writes nothing. |
| `organizer_time_to_value` | — | `{ key, at }[]` | Local first-touch milestone timestamps for diagnostics. |
| `organizer_verify_audit_chain` | — | `ChainVerification` | Verify the tamper-evidence chain offline. |

`ExecutionResult = { execution_id, report }` where `report` is the
`ExecutionReport` (`applied`, `skipped`, `failed`, `audit`, `undo`).

`ChainVerification = { intact: bool, sealed_count, unsealed_count,
first_break: { execution_id, reason } | null }`.

## Tamper-evidence & retention

Each execution is sealed as it is saved: a SHA-256 `hash` over the run's stored
row bytes plus the previous run's hash, forming a chain ordered by insertion
sequence (reliable insertion order — `created_at` is second-granular and `id`
is a random UUID). `organizer_verify_audit_chain` walks the chain and confirms each
sealed run still matches its seal and links to the one before it, so altered
history is detectable offline. Rows written before the V5 migration are
"unsealed" (empty hash) and counted separately, never failing verification.

Retention is user-set via `config.audit` (`retention_keep_last` /
`retention_keep_days`, both default `null` = keep all); `organizer_execute`
prunes an oldest prefix after each run, which preserves the retained suffix's
chain contiguity. Pruning is the user deleting their own history — opt-in, never
silent, consistent with the privacy stance.

The frontend's **Guided setup** button (`organizerRunWizard` in `src/main.js`)
composes `organizer_create_zone` + `organizer_add_folder_rule` from a short
interview into one of three presets (Client filing, Bookkeeping inbox, Downloads
cleanup) — no dedicated backend command; the wizard is pure UI over these two.

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
- **FolderRule** — `{ path, can_read, can_create, can_rename, can_move,
  can_copy, can_delete, trust }`. `trust` is `automate | ask_first | never`
  (snake_case) and serde-defaults to `ask_first` when absent, so rules and
  frontends predating trust keep their behavior.
- **PlanAction** carries an optional `rule_path` naming the boundary that fired.
- **AuditLog** serializes transparently as an array of
  `{ capability, outcome, at }` events, each optionally carrying `rule_path`
  (the boundary that fired) and `provenance` (`automated` | `user_approved`).
  Both are omitted when absent, so older logs keep their exact shape.
- **UndoReport** = `{ reverted, skipped, failed }`.

## Registration

Commands are re-exported from `src-tauri/src/commands.rs` (`mod organizer;
pub use organizer::*;`) and listed in the `generate_handler!` macro in
`src-tauri/src/lib.rs`.
