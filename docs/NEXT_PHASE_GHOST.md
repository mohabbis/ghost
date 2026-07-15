# Next Phase Plan for Ghost

## Executive decision

Ghost should stop expanding as a broad desktop AI automation demo and become a trusted local automation layer for repetitive computer work.

The next phase is not more novelty. The next phase is trust infrastructure.

Strategic product definition:

> Ghost turns repeated computer work into safe, reusable, permission-bounded routines.

First flagship wedge:

> Ghost Organizer: safely clean, classify, rename, and move files with preview, approval, audit, and undo.

## Why this direction

The current repository already has useful foundations: Tauri 2, Rust backend, macOS and Windows platform support, local workflow storage, recording/replay concepts, local auth, diagnostics, and experimental AI/cloud/observer surfaces.

The problem is not lack of features. The problem is product trust.

A product that can affect a user's real computer must be boringly clear about what it can do, where it can do it, and how the user can reverse it. Broad automation should wait until the safe execution model is real.

## Product thesis

People do not need another chatbot. They need a trusted system that understands repetitive computer work, turns it into reviewable routines, and executes those routines only inside boundaries they control.

Ghost's moat should be:

- local-first control
- permission-bounded operations
- inspectable plans
- safe deterministic execution
- undoable changes
- audit history
- human approval before meaningful changes
- memory of user-approved patterns and preferences

AI is useful, but AI is not the core product. AI suggests. Ghost verifies. The user approves. The deterministic core executes.

## Canonical pipeline

Every meaningful operation should pass through:

```text
Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
```

No shortcut should exist for file operations, workflow replay, browser actions, network actions, or any future app integration.

## Phase 0: Repository and documentation reset

Status: partially complete.

Completed / in progress:

- `CLAUDE.md` reset toward local-first trust strategy.
- `AGENTS.md` added for coding-agent guardrails.
- PR #57 merged command modularization by product boundary.
- `docs/command-registry.md` added.
- `docs/core-boundaries.md` exists and correctly separates stable core from experimental surfaces.

Remaining tasks:

1. Add command risk inventory.
2. Add policy engine design doc.
3. Add Ghost Organizer product spec.
4. Add SQLite/storage migration plan.
5. Add release trust checklist for signing/notarization.

Acceptance criteria:

- Future agents can identify stable vs experimental work from docs alone.
- Every command has a module, stability classification, and risk classification.
- README, marketing copy, and docs no longer over-position Ghost as broad autonomous control.

## Phase 1: Command risk inventory

Goal: make the current command surface reviewable.

Create or expand `docs/command-registry.md` with a table:

| Command | Module | Stability | Files | OS interaction | Screen data | Network | Auth/secrets | Risk | Notes |
|---|---|---|---:|---:|---:|---:|---:|---|---|
| `start_recording` | core | stable | no | yes | no | no | no | high | Requires visible active state. |
| `replay_workflow` | core | stable | maybe | yes | no | no | no | critical | Must route through policy before broad use. |
| `save_workflow` | core | stable | yes | no | no | no | maybe | medium | Encrypted when local auth configured. |
| `auth_unlock` | auth | stable | yes | no | no | no | yes | high | Local-only. |
| `generate_workflow_from_prompt` | experimental | experimental | indirect | indirect | maybe | maybe | no | critical | Suggestion-only future. |
| `init_cloud_sync` | experimental | experimental | maybe | no | no | yes | maybe | critical | Hidden from MVP. |
| `start_observer` | experimental | experimental | maybe | maybe | maybe | no | no | critical | Hidden from MVP. |

Rules:

- Stable commands must have documented failure modes.
- High and critical commands require explicit approval, developer-only gating, or removal from default UI.
- Experimental commands cannot graduate until they have tests and documented limits.

Acceptance criteria:

- Every command is inventoried.
- Experimental commands are visibly isolated.
- New command PRs must update the registry.

## Phase 2: Policy engine skeleton

Goal: introduce the central trust mechanism before adding new product features.

Suggested modules:

```text
src-tauri/src/policy/
  mod.rs
  capability.rs
  decision.rs
  engine.rs
  risk.rs
  zone.rs
```

Core types:

```rust
pub enum Capability {
    ReadFolder { path: PathBuf },
    CreateFolder { path: PathBuf },
    RenameFile { from: PathBuf, to: PathBuf },
    MoveFile { from: PathBuf, to: PathBuf },
    CopyFile { from: PathBuf, to: PathBuf },
    DeleteFile { path: PathBuf },
    StartRecording,
    ReplayWorkflow { workflow_id: String },
    CaptureScreen,
    UseNetwork { host: String },
    GenerateWorkflowFromPrompt,
}

pub enum PolicyDecision {
    Allow,
    Deny { reason: String },
    RequireConfirmation { reason: String, risk: RiskLevel },
}

pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
```

MVP policy defaults:

- Allow reading approved source folders.
- Allow creating folders inside approved destination folders.
- Allow moving/renaming files inside approved boundaries after approval.
- Require confirmation for low-confidence or bulk changes.
- Deny delete operations.
- Deny operations outside approved folders.
- Deny network, cloud, email, browser, and app actions for Organizer MVP.

Acceptance criteria:

- Unit tests cover allow, deny, and confirmation-required decisions.
- Delete file is denied in MVP.
- Move outside approved destination is denied.
- Bulk move is at least medium risk.
- Low-confidence rename requires confirmation.

## Phase 3: Zones and local storage

Goal: define where Ghost is allowed to work.

A Zone is a user-approved boundary. For the Organizer MVP, a Zone can start as source folders, destination folders, allowed operation types, and default decision.

SQLite tables:

```sql
CREATE TABLE zones (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  default_decision TEXT NOT NULL CHECK (default_decision IN ('deny', 'ask', 'allow')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE zone_folder_rules (
  id TEXT PRIMARY KEY,
  zone_id TEXT NOT NULL REFERENCES zones(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  can_read INTEGER NOT NULL DEFAULT 1,
  can_create INTEGER NOT NULL DEFAULT 0,
  can_rename INTEGER NOT NULL DEFAULT 0,
  can_move INTEGER NOT NULL DEFAULT 0,
  can_copy INTEGER NOT NULL DEFAULT 0,
  can_delete INTEGER NOT NULL DEFAULT 0
);
```

Acceptance criteria:

- User can create a local Zone.
- Zone folder rules persist locally.
- Policy engine can evaluate folder operations against Zone rules.
- Database migrations are versioned.

## Phase 4: Ghost Organizer planner

Goal: generate a safe, reviewable file organization plan without mutating files.

Suggested modules:

```text
src-tauri/src/organizer/
  mod.rs
  scanner.rs
  classifier.rs
  planner.rs
  naming.rs
  conflict.rs
```

Planner flow:

1. Load selected Zone.
2. Scan allowed source folders.
3. Filter ignored/system files.
4. Extract metadata.
5. Classify deterministically first.
6. Generate target folders.
7. Generate safe filenames.
8. Detect conflicts.
9. Create plan actions.
10. Evaluate actions through policy.
11. Return preview.

Deterministic classification inputs:

- filename
- extension
- folder location
- created/modified dates
- file size
- known category/course/project names
- aliases
- PDF metadata where available
- previous user corrections later

Acceptance criteria:

- Planner does not mutate files.
- Planner returns confidence scores and reasons.
- Planner detects conflicts.
- Planner produces policy decisions per action.
- Low-confidence files are reviewable.

## Phase 5: Plan preview and approval UI

Goal: make trust visible.

Approval screen must show:

- what Ghost will do
- where Ghost will do it
- what Ghost will not do
- risk level
- low-confidence items
- conflict items
- undo availability

Example summary:

```text
Ghost is ready to organize 18 files.

Allowed area:
Downloads -> Documents/School

Planned changes:
- Create 4 folders
- Rename 12 files
- Move 18 files
- Delete 0 files
- Upload 0 files
- Send 0 files

Needs review:
- 3 files have low confidence

Undo:
Available after completion
```

Acceptance criteria:

- User can approve, cancel, or revise.
- Low-confidence items can be corrected manually.
- No plan executes without explicit approval.
- UI does not expose broad experimental features by default.

## Phase 6: Organizer executor, audit, and undo

Goal: safely apply approved file plans.

Suggested modules:

```text
src-tauri/src/organizer/executor.rs
src-tauri/src/organizer/undo.rs
src-tauri/src/audit/audit_log.rs
src-tauri/src/audit/undo_journal.rs
```

Executor flow:

1. Receive approved plan.
2. Verify plan still matches current file state.
3. Re-check policy.
4. Lock plan.
5. Write undo entry before each reversible operation.
6. Apply operation.
7. Verify result.
8. Write audit event.
9. Mark plan completed or failed.
10. Show summary.

MVP allowed operations:

- create folder
- move file
- rename file
- copy file only if explicitly included

MVP denied operations:

- delete file
- overwrite file
- network upload
- email/browser actions
- background automation

Acceptance criteria:

- Every applied action writes audit.
- Every reversible action writes undo before execution.
- Undo runs in reverse order.
- Partial failure leaves recoverable state.
- No silent overwrite.
- No deletion.

## Phase 7: Release trust

Goal: make installs trustworthy before public positioning.

Tasks:

- Configure macOS Developer ID signing.
- Configure macOS notarization.
- Add Windows code signing plan.
- Add artifact smoke checks where feasible.
- Remove quarantine-bypass messaging from consumer-facing docs once signing is fixed.

Acceptance criteria:

- macOS release is signed and notarized.
- Windows release is signed or clearly marked developer preview.
- README does not ask normal users to bypass OS trust warnings for public launch.

## Phase 8: Recorded routines after Organizer

Only resume broad routine work after Organizer proves the trust model.

Routine requirements:

- bind routine to Zone
- semantic UI target first, coordinate fallback second
- active app/window verification
- visible active state
- emergency stop
- step preview before replay
- policy check before replay
- audit after replay
- no sensitive actions by default

Acceptance criteria:

- Coordinate-only routines are marked fragile.
- Wrong app/window blocks replay.
- High-risk actions require explicit confirmation.
- Critical actions remain blocked unless future policy explicitly supports them.

## Phase 9: AI suggestions

AI should enter after deterministic planning works.

Allowed AI roles:

- suggest categories
- suggest filenames
- summarize plans
- explain classification reasons
- propose routine drafts
- detect possible recurring cleanup patterns

Forbidden AI roles:

- direct execution
- direct approval
- direct file mutation
- direct send/submit/upload actions
- bypassing policy

Acceptance criteria:

- AI suggestions validate against schema.
- AI suggestions pass policy before becoming plan actions.
- User sees reasons and confidence.
- AI can be disabled without breaking core Organizer.

## What to freeze now

Freeze from default product work:

- cloud sync
- workspaces
- observer mode
- geek insights
- enterprise dashboards
- prompt-generated executable routines
- visual-regression workflow replay
- data-source workflow testing

These may remain in experimental modules, but they should not drive the next product phase.

## What to build now

Build in this order:

1. Command risk inventory.
2. Policy engine skeleton.
3. Zone model and folder rules.
4. Organizer scanner/classifier/planner.
5. Plan preview UI.
6. Approval flow.
7. Organizer executor.
8. Audit log.
9. Undo journal.
10. Release trust improvements.

## North-star sentence

Ghost may suggest anything, but it may only do what the user approved inside a boundary the user controls.
