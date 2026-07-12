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
  `FolderRule` grants per-folder `can_read/create/rename/move/copy/delete` and
  carries a **`trust: TrustLevel`** (see below). These are pure domain types;
  persistence lives in `src-tauri/src/storage/` (storage depends on policy,
  never the reverse).
- **`TrustLevel`** (`zone.rs`) — `Automate | AskFirst | Never`, the user-facing
  control over how much autonomy a rule's *mutating* grants carry. `AskFirst`
  is the default (and the serde/storage default), so rules created before trust
  levels existed keep their original behavior. On a two-sided operation
  (move/rename/copy) the **stricter** of the two rules' trust levels governs.
- **`RiskLevel`** (`risk.rs`) — `Low | Medium | High | Critical`. Distinct from
  `core::guard::GuardSeverity`, which classifies recorded-workflow findings.
- **`PolicyDecision`** (`decision.rs`) — `Allow`, `Deny { reason }`, or
  `RequireConfirmation { reason, risk }`. The deterministic core executes only
  `Allow`; `RequireConfirmation` must surface to the user; `Deny` stops the action.
  The wire shape (internally-tagged `decision`, snake_case) is a pinned frontend
  contract — extend additively only.
- **`evaluate(cap, rules) -> PolicyDecision`** (`engine.rs`) — the whole engine.
- **`evaluate_with_attribution(cap, rules) -> Evaluation`** (`engine.rs`) —
  same decision plus the `rule_path` of the `FolderRule` that fired (`None` when
  nothing matched). The planner and executor use this so the plan preview and
  audit log can name *which* boundary authorized or refused each action.

Containment is **component-aware** (`Path::starts_with`), so `/a/bc` is correctly
not treated as inside `/a/b`. A raw string prefix check would leak access to
sibling folders.

## MVP decision table (Ghost Organizer)

| Capability | Decision | Notes |
|---|---|---|
| `ReadFolder` | Allow iff a `can_read` rule contains the path | else Deny. Reads are allowed at every trust level — the read grant *is* the scope, and planning needs to read. |
| `CreateFolder` | Allow iff a `can_create` rule contains the path (any trust except `Never`) | `Never` refuses; else Deny |
| `RenameFile` / `MoveFile` | Governed by the **stricter** covering rule's trust: `Automate` → Allow, `AskFirst` → RequireConfirmation (Medium), `Never` → Deny — iff both `from` and `to` are inside rules granting the permission | else Deny (crosses boundary) |
| `CopyFile` | Same trust mapping, iff `from` is `can_read` and `to` is `can_copy` | else Deny |
| `DeleteFile` | **Always Deny** | the MVP never deletes files, even under `Automate` |
| `CaptureScreen`, `UseNetwork`, `GenerateWorkflowFromPrompt`, `StartRecording`, `ReplayWorkflow` | Deny | outside Organizer scope; deny-by-default |

## Invariants

- **Deny by default.** Anything not covered by a granting rule is refused.
- **No deletes, no overwrites** in the Organizer MVP — `DeleteFile` is denied at
  every trust level, so `Automate` never widens what Ghost is willing to do; it
  only removes the confirmation prompt for the mutations already permitted.
- **Trust is enforced in the engine, never trusted from the frontend.** The UI
  can set a rule's trust (`organizer_set_rule_trust`), but the executor
  re-evaluates every action server-side, so a tampered plan can't escalate.
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
