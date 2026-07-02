# Policy Engine

The policy engine is Ghost's central trust mechanism. Its rule, in one sentence:

> Ghost may suggest anything, but it may only do what the user approved inside a
> boundary the user controls.

Every meaningful operation flows through the pipeline:

```text
Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
```

The engine sits at the **Policy** step. It lives in `src-tauri/src/policy/` and
is **pure and deny-by-default**: it performs no IO, has no knowledge of storage,
and refuses anything not explicitly permitted by an active rule. That makes it
trivially unit-testable without the UI or a database.

## Model

- **`Capability`** (`capability.rs`) — the concrete action a plan wants to take
  (`ReadFolder`, `CreateFolder`, `RenameFile`, `MoveFile`, `CopyFile`,
  `DeleteFile`, plus the out-of-Organizer-scope `StartRecording`,
  `ReplayWorkflow`, `CaptureScreen`, `UseNetwork`, `GenerateWorkflowFromPrompt`).
- **`FolderRule`** / **`Zone`** (`zone.rs`) — a user-approved boundary. A
  `FolderRule` grants per-folder `can_read/create/rename/move/copy/delete`. These
  are pure domain types; persistence lives in `src-tauri/src/storage/` (storage
  depends on policy, never the reverse).
- **`RiskLevel`** (`risk.rs`) — `Low | Medium | High | Critical`. Distinct from
  `core::guard::GuardSeverity`, which classifies recorded-workflow findings.
- **`PolicyDecision`** (`decision.rs`) — `Allow`, `Deny { reason }`, or
  `RequireConfirmation { reason, risk }`. The deterministic core executes only
  `Allow`; `RequireConfirmation` must surface to the user; `Deny` stops the action.
- **`evaluate(cap, rules) -> PolicyDecision`** (`engine.rs`) — the whole engine.

Containment is **component-aware** (`Path::starts_with`), so `/a/bc` is correctly
not treated as inside `/a/b`. A raw string prefix check would leak access to
sibling folders.

## MVP decision table (Ghost Organizer)

| Capability | Decision | Notes |
|---|---|---|
| `ReadFolder` | Allow iff a `can_read` rule contains the path | else Deny |
| `CreateFolder` | Allow iff a `can_create` rule contains the path | else Deny |
| `RenameFile` / `MoveFile` | RequireConfirmation (Medium) iff both `from` and `to` are inside rules granting the permission | else Deny (crosses boundary) |
| `CopyFile` | RequireConfirmation (Medium) iff `from` is `can_read` and `to` is `can_copy` | else Deny |
| `DeleteFile` | **Always Deny** | the MVP never deletes files |
| `CaptureScreen`, `UseNetwork`, `GenerateWorkflowFromPrompt`, `StartRecording`, `ReplayWorkflow` | Deny | outside Organizer scope; deny-by-default |

## Invariants

- **Deny by default.** Anything not covered by a granting rule is refused.
- **No deletes, no overwrites** in the Organizer MVP.
- **AI never executes.** `GenerateWorkflowFromPrompt` is denied as a *direct*
  capability; AI output may only become plan actions that are themselves
  re-evaluated by the engine.
- The engine is **storage-agnostic**: callers load `FolderRule`s (e.g. from
  `storage::zones::list_folder_rules`) and pass them in.

## Status

Implemented: capability/decision/risk/zone types and `evaluate`, with unit tests
covering allow, deny, confirmation, the delete ban, and the sibling-prefix
boundary case. The Ghost Organizer planner and executor use this engine for
every proposed and executed file action, and the exposed Organizer commands are
inventoried in `docs/command-registry.md`.
