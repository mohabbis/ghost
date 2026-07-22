# The Trust Pipeline: Ghost's Core Architecture

## Overview

Every meaningful operation in Ghost flows through a deterministic, user-controlled pipeline:

```
Intent → Plan → Policy Check → Approval → Execution → Audit → Undo
```

This is not a feature. This is the product.

The pipeline ensures:
- **Visibility** — Users see exactly what will happen before anything runs.
- **Control** — Users choose what gets approved.
- **Policy enforcement** — Ghost Guard blocks risky operations by default.
- **Auditability** — Every step is logged with full context.
- **Reversibility** — Undo data is written before destructive operations.

## Stage 1: Intent

**What enters**: User action or workflow trigger.

**Examples**:
- "Organize this Downloads folder"
- "Move invoices to client folders"
- "Rename these files with date prefixes"
- A saved workflow scheduled for replay

**What Ghost does**:
- Receives input from user (folder selection, workflow choice).
- Validates that the intent is supported.
- Checks that required permissions are available (is this Zone configured? Are these Capabilities allowed?).

**What must be logged**:
- Intent ID (unique per user action).
- Timestamp.
- User action trigger (UI button, scheduled, etc.).
- Requested intent type.

**What can fail**:
- User selects a Zone not previously configured.
- User selects a workflow that is not accessible.
- User requests an operation not yet supported.

**Policy check at this stage**: Does the user have permission to attempt this action? (Usually yes; this is veto-only.)

---

## Stage 2: Plan

**What enters**: Intent + context (folder contents, workflow definition, etc.).

**What Ghost does**:

For **Ghost Organizer**:
1. Scan folder (list files, read metadata).
2. Classify each file against deterministic rules (filename patterns, extensions, metadata).
3. Propose file moves and renames.
4. Detect conflicts (destination file exists, would overwrite, etc.).
5. Build a structured Plan object.

For **recorded workflows** (future):
1. Compress raw input events into semantic steps.
2. Resolve UI targets (element name/role, window context).
3. Flag coordinate-only targets and low-confidence steps.
4. Build the step timeline.

**What must be included in the Plan**:
- Plan ID (unique).
- Timestamp.
- Source (folder path, workflow ID, etc.).
- Individual proposed actions:
  - Source path.
  - Destination path.
  - Proposed rename (if any).
  - Confidence score (0.0–1.0).
  - Matched rule(s).
  - Risk classification (allow, warn, require approval, block).
- Aggregated stats:
  - Total files analyzed.
  - Files to move.
  - Files to rename.
  - Files to skip.
  - Files with low confidence.
  - Conflicts detected.
  - Policy blocks.
- Undo journal reference (created but not written yet).

**The Plan is read-only at this stage.** No files are touched. No mutations happen.

**What can fail**:
- Folder is inaccessible.
- File system is corrupted or has permission issues.
- Plan is too complex (e.g., > 10,000 files—threshold TBD).

**Recovery**: If planning fails partway, the failure point is reported, and the user can retry or cancel. No changes have been made.

---

## Stage 3: Policy Check

**What enters**: Plan.

**Ghost Guard evaluates**:

For each proposed action, classify as:

| Decision | Meaning | Action |
|----------|---------|--------|
| `Allow` | Safe, inside approved boundaries, no conflicts | Execute if approved |
| `Warn` | Probably safe, but user should review | Highlight for approval |
| `RequireApproval` | Higher-risk operation | Require explicit user confirmation |
| `Block` | Not allowed by policy | Remove from plan; log block reason |

### Policy Rules (Deny by Default)

**Organizer-specific blocks**:
- Delete file (even if empty) → Block
- Move file outside Zone → Block
- Move system directories (~/Library, /System, etc.) → Block
- Overwrite existing file without explicit approval → RequireApproval
- Modify hidden files (`.filename`) → Warn
- Batch operation > 100 files → Warn

**General blocks**:
- Insufficient permissions → Block
- Operation requires ungranted Capability → Block
- File size > threshold → Block (default: 1 GB)
- Pattern suggests secrets (contains "password", "key", "secret") → Block

### Confidence-based decisions:
- Classification confidence < 0.7 → Warn (may suggest skipping)
- Coordinate-only target in replay → Warn

**What must be output**:

For each action in the Plan:
- Policy decision (Allow/Warn/RequireApproval/Block)
- Reason
- Risk class
- Timestamp

Aggregated summary:
- Count of allowed actions.
- Count of warned actions.
- Count of blocked actions.
- Total mutations if approved.

**This stage does not modify the Plan.** It annotates it with policy decisions.

**What can fail**: Policy engine crashes or enters invalid state (should never happen; caught in unit tests).

---

## Stage 4: Approval

**What enters**: Plan + Policy decisions.

**UI responsibility**:

Ghost presents to the user:
1. **Before/after tree** — Visual folder structure showing proposed changes.
2. **Summary** — Files to move, rename, skip, and any conflicts.
3. **Action list** — Detailed table with columns:
   - File name
   - Current location
   - Proposed destination
   - Proposed rename
   - Confidence
   - Policy decision (✅ Allow / ⚠️ Warn / 🔒 Block)
   - Matched rule(s)

4. **User actions**:
   - "Approve All" — Execute all allowed actions.
   - "Deselect" — Remove specific actions from the plan.
   - "Edit destination" — Change where a file goes.
   - "Edit name" — Change the proposed rename.
   - "View reason" — Expand warnings and policy details.
   - "Save as preset" — Store this plan for future use.
   - "Export plan" — Save as JSON before execution.
   - "Cancel" — Abort and return to intent stage.

**Approval record**:
- Approval ID (unique).
- User ID (if auth is implemented).
- Timestamp.
- Actions approved.
- Actions deselected.
- Edits made to destinations/names.
- Final plan hash.

**This is the trust boundary.** The user explicitly sees and approves the operations. If they don't approve, nothing happens.

**What can fail**:
- User has second thoughts → can cancel (no harm done).
- Plan is stale (files were moved/deleted between planning and approval) → user is asked to re-plan.

**If approval fails or is cancelled**: Return to Intent stage. Offer to re-plan or give up.

---

## Stage 5: Execution

**Pre-execution checks**:
1. Re-validate approval is current.
2. Re-check file accessibility (files still exist, still accessible).
3. Verify undo journal can be written (disk space, permissions).
4. If any check fails, block execution. Report the failure. Do not proceed.

**Undo journal write** (critical):

Before ANY file mutation, Ghost writes an undo journal entry:

```json
{
  "journal_id": "undo-20260706-001",
  "run_id": "run-20260706-001",
  "timestamp": "2026-07-06T14:32:00Z",
  "operations": [
    {
      "operation": "move",
      "source": "/Users/me/Downloads/invoice_acme_june.pdf",
      "destination": "/Users/me/Documents/Clients/Acme/Invoices/2026/",
      "destination_name": "invoice_acme_june.pdf",
      "file_hash_before": "sha256:abc123...",
      "size": 245120
    },
    {
      "operation": "rename",
      "source": "/Users/me/Downloads/statement.pdf",
      "old_name": "statement.pdf",
      "new_name": "2026-06_Chase_Statement.pdf",
      "destination": "/Users/me/Documents/Statements/",
      "file_hash_before": "sha256:def456..."
    },
    {
      "operation": "create_folder",
      "path": "/Users/me/Documents/Clients/Acme/Invoices/2026/"
    }
  ]
}
```

If undo journal write fails, execution is aborted. Report the failure.

**Execution loop**:

For each approved action:
1. Re-check policy for this specific action (in case state changed).
2. Perform the operation (move, rename, create folder).
3. Record the result (success, failure, reason).
4. Append to execution log.
5. Update progress UI.
6. If operation fails:
   - Log the failure.
   - Mark the step as failed.
   - Continue with next operation (resilient mode).
   - Or, stop and offer resume (strict mode).

**What must be logged during execution**:
- Execution Run ID (unique per run).
- Timestamp.
- For each step:
  - Step ID.
  - Operation type.
  - Source and destination.
  - Success (true/false).
  - Error code (if failed).
  - Duration (ms).

**What can fail**:
- Permission denied on a file.
- File disappeared between planning and execution.
- Destination path is no longer writable.
- Conflict: destination file exists and can't be overwritten.
- Disk full.
- File is locked by another process.

**Failure recovery**:
- If a single file fails, log it and continue (resilience).
- If many files fail (> 10%), offer resume or undo.
- If critical failure (disk full, Zone corrupted), abort and undo.

**UI during execution**:
- Live per-step progress.
- Current operation being performed.
- Number of files completed / remaining.
- Estimated time remaining.
- Cancel button (always available).
- Pause/resume (if supported).

---

## Stage 6: Audit

**What enters**: Completed execution.

**Audit log entry** (immutable):

Ghost appends an entry to the append-only audit log:

```json
{
  "entry_id": "audit-20260706-001",
  "parent_hash": "sha256:prev_entry_hash",
  "current_hash": "sha256:hash_of_this_entry",
  "run_id": "run-20260706-001",
  "timestamp": "2026-07-06T14:32:42Z",
  "workflow": "organizer",
  "zone_id": "zone-downloads",
  "operation_count": 81,
  "operations_summary": {
    "moved": 43,
    "renamed": 31,
    "created_folders": 5,
    "skipped": 2,
    "blocked": 0
  },
  "success": true,
  "duration_ms": 3241,
  "approval_id": "approval-20260706-001",
  "user_id": "local",
  "undo_journal_id": "undo-20260706-001",
  "failures": []
}
```

**Hash chain integrity** (tamper-evident):

Each audit entry includes:
- Hash of the previous entry (`parent_hash`).
- Hash of the current entry (`current_hash`).

This creates a tamper-evident chain. If any entry is modified, the hashes break, and the tampering is obvious.

**Export formats**:
- **CSV** (for the ops lead or an admin):
  - Timestamp, Workflow, Operations summary, Success, Duration, Undo available
- **JSON** (for technical users):
  - Full audit entry with all fields
- **PDF** (future):
  - Human-readable compliance report

**Audit log guarantees**:
- Append-only (no deletion or modification).
- Stored locally (no cloud upload).
- User can export at any time.
- Hash chain validates integrity.

---

## Stage 7: Undo

**What enters**: Completed execution + Undo journal.

**Undo availability**:

After a successful execution, the user sees:
- "Undo" button (available for 24 hours or until next run).
- Audit entry with undo journal reference.
- Summary of what can be undone.

**Undo execution**:

1. Load undo journal.
2. Validate journal integrity (hash checks).
3. Reverse operations in reverse order:
   - Rename back to original name.
   - Move files back to original location.
   - Remove created folders (if empty).
4. Create new audit entry:
   - Operation: `undo`
   - Reference to original run ID.
   - Reference to original undo journal.
5. User is shown: "Run undone. 81 files restored."

**What cannot be undone**:
- Files deleted by the user between execution and undo (we have the hash, but can't recover deleted files).
- Files modified by the user or another app.
- Folders that have been manually moved.

**Undo journal guarantees**:
- Written before execution (atomic).
- Never modified after execution.
- Hash-chained with audit entry.
- User can review before undoing.

---

## Failures and Recovery

### Partial Execution

If execution stops at step 50 of 100:

1. **Continue** — Complete remaining steps.
2. **Resume from failed step** — Retry the failed operation, then continue.
3. **Undo completed steps** — Reverse what was done, return to original state.

### Conflict During Execution

If destination file already exists:

1. **Skip** — Leave the file as-is, move to next.
2. **Rename** — Append timestamp or counter (`invoice_2026-07-06_1.pdf`).
3. **Overwrite** — Replace the destination (requires approval).
4. **Undo** — Reverse what was done so far, return to original state.

### Policy Block During Execution

If Ghost Guard detects a blocked operation after approval (e.g., file grew above size threshold):

1. **Skip** — Remove from execution, log reason.
2. **Warn user** — Show which files were blocked and why.
3. **Undo** — Reverse what was done so far.

---

## The Trust Principle

**One rule, enforced everywhere:**

> Ghost may suggest anything, but it only does what you have explicitly approved, inside boundaries you control.

This is enforced in code:

1. **Intent** is user-controlled.
2. **Plan** is read-only (no mutations).
3. **Policy** is deny-by-default.
4. **Approval** is explicit and user-driven.
5. **Execution** follows the approved plan exactly.
6. **Audit** is immutable and exportable.
7. **Undo** is always available for reversible operations.

No shortcuts. No hidden mutations. No autonomous behavior. Every meaningful operation passes through all seven stages.
