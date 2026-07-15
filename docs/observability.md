# Observability & Audit Export

## Structured Logging

Ghost uses structured JSON logging across the trust pipeline for auditability and debugging.

### Log Levels

| Level | Usage | Example |
|-------|-------|---------|
| `ERROR` | Failures that require user attention | "Zone path is not accessible" |
| `WARN` | Risky operations that completed | "Operation required user approval" |
| `INFO` | Normal workflow events | "Plan generated: 43 files to move" |
| `DEBUG` | Developer-level events | "Policy evaluated: 2 blocked, 1 warned" |
| `TRACE` | Verbose internal events | "Resolving click target via accessibility API" |

### Core Pipeline Logging

Every operation logs entry/exit and decision:

```json
{
  "timestamp": "2026-07-06T14:32:00Z",
  "level": "INFO",
  "stage": "organizer_plan",
  "event": "plan_generated",
  "zone_id": "zone-downloads",
  "files_analyzed": 50,
  "files_proposed": 43,
  "files_skipped": 7,
  "policy_evaluated": true,
  "duration_ms": 1243
}
```

### Stage-by-Stage Logs

#### Intent

```json
{
  "timestamp": "2026-07-06T14:32:00Z",
  "stage": "intent",
  "event": "organizer_selected",
  "zone_id": "zone-downloads",
  "user_action": "button_click"
}
```

#### Plan

```json
{
  "timestamp": "2026-07-06T14:32:01Z",
  "stage": "plan",
  "event": "plan_generated",
  "plan_id": "plan-001",
  "actions": [
    {
      "action_id": "act-001",
      "type": "move",
      "source": "invoice.pdf",
      "destination": "Invoices/2026/",
      "confidence": 0.95,
      "policy_class": "allow"
    }
  ],
  "duration_ms": 1200
}
```

#### Policy Check

```json
{
  "timestamp": "2026-07-06T14:32:02Z",
  "stage": "policy",
  "event": "policy_evaluated",
  "plan_id": "plan-001",
  "decisions": {
    "allow": 43,
    "warn": 0,
    "require_approval": 0,
    "block": 0
  },
  "duration_ms": 50
}
```

#### Approval

```json
{
  "timestamp": "2026-07-06T14:32:15Z",
  "stage": "approval",
  "event": "plan_approved",
  "approval_id": "approval-001",
  "plan_id": "plan-001",
  "actions_approved": 43,
  "actions_deselected": 0,
  "time_to_approval_ms": 13000
}
```

#### Execution

```json
{
  "timestamp": "2026-07-06T14:32:16Z",
  "stage": "execution",
  "event": "execution_started",
  "run_id": "run-001",
  "plan_id": "plan-001",
  "total_actions": 43
}
```

Per-action logs:

```json
{
  "timestamp": "2026-07-06T14:32:17Z",
  "stage": "execution",
  "event": "action_executed",
  "run_id": "run-001",
  "action_id": "act-001",
  "type": "move",
  "source": "invoice.pdf",
  "destination": "Invoices/2026/invoice.pdf",
  "success": true,
  "duration_ms": 45
}
```

#### Audit

```json
{
  "timestamp": "2026-07-06T14:32:42Z",
  "stage": "audit",
  "event": "audit_recorded",
  "run_id": "run-001",
  "audit_entry_id": "audit-001",
  "operations": {
    "moved": 43,
    "renamed": 0,
    "created_folders": 5,
    "skipped": 2,
    "blocked": 0
  },
  "total_duration_ms": 3200,
  "hash_valid": true
}
```

#### Undo

```json
{
  "timestamp": "2026-07-06T14:35:00Z",
  "stage": "undo",
  "event": "undo_initiated",
  "run_id": "run-001",
  "undo_journal_id": "undo-001",
  "actions_to_reverse": 43
}
```

Per-action undo:

```json
{
  "timestamp": "2026-07-06T14:35:01Z",
  "stage": "undo",
  "event": "undo_action",
  "run_id": "run-001",
  "operation": "restore",
  "source": "Invoices/2026/invoice.pdf",
  "destination": "invoice.pdf",
  "success": true
}
```

### Error Logging

Every error includes context:

```json
{
  "timestamp": "2026-07-06T14:32:35Z",
  "level": "ERROR",
  "stage": "execution",
  "event": "action_failed",
  "run_id": "run-001",
  "action_id": "act-040",
  "type": "move",
  "error_code": "PERMISSION_DENIED",
  "error_message": "Cannot move file: Permission denied",
  "source": "receipt.pdf",
  "destination": "Receipts/2026/",
  "recovery_available": true
}
```

---

## Audit Export

### CSV Export

For spreadsheet analysis by bookkeepers and admins:

**Filename**: `ghost-audit-YYYY-MM-DD.csv`

**Columns**:

```
Timestamp,Workflow,Run ID,Operation,Source,Destination,Policy Decision,Status,Undo Available,Hash
2026-07-06T14:32:17Z,Organizer,run-001,Move,invoice.pdf,Invoices/2026/invoice.pdf,Allow,Success,Yes,abc123...
2026-07-06T14:32:18Z,Organizer,run-001,Create Folder,,,Allow,Success,Yes,def456...
```

**Usage**: Import into Excel / Google Sheets for analysis.

### JSON Export

For programmatic access:

**Filename**: `ghost-audit-YYYY-MM-DD.json`

**Structure**:

```json
{
  "export_date": "2026-07-06T14:35:00Z",
  "ghost_version": "1.0.0",
  "runs": [
    {
      "run_id": "run-001",
      "workflow": "organizer",
      "start_time": "2026-07-06T14:32:16Z",
      "end_time": "2026-07-06T14:32:42Z",
      "duration_ms": 3200,
      "status": "success",
      "operations": [
        {
          "timestamp": "2026-07-06T14:32:17Z",
          "type": "move",
          "source": "invoice.pdf",
          "destination": "Invoices/2026/invoice.pdf",
          "success": true,
          "policy_decision": "allow",
          "hash": "abc123...",
          "parent_hash": "000000...",
          "undo_available": true
        }
      ],
      "summary": {
        "moved": 43,
        "created_folders": 5,
        "skipped": 2,
        "blocked": 0,
        "errors": 0
      },
      "undo_journal_id": "undo-001"
    }
  ]
}
```

**Usage**: API integrations, compliance reporting, data analysis.

### Human-Readable Report (Future)

Markdown or PDF format for compliance and stakeholder reporting:

```markdown
# Ghost Audit Report
## 2026-07-06

- **Workflows Run**: 1
- **Total Operations**: 50
- **Success Rate**: 100%
- **Undo Used**: 0
- **Policy Blocks**: 0

### Run: Organizer (run-001)
- **Duration**: 3.2 seconds
- **Operations**:
  - Moved: 43 files
  - Created: 5 folders
  - Skipped: 2 (low confidence)
- **Status**: Success
- **Undo**: Available
```

---

## Log Retention & Storage

### Local Storage

- **Location**: `~/.ghost/logs/` (or platform default)
- **Format**: JSON (one entry per operation)
- **Retention**: 90 days (configurable)
- **Size**: ~100 KB per 1,000 operations

### User Control

```
Settings → Privacy & Logging
- [ ] Enable audit logging (default: ON)
- [ ] Enable debug logging (default: OFF)
- [ ] Retain logs for: 30 / 90 / 365 days
- [ ] [Export Logs...] → CSV / JSON
- [ ] [Delete All Logs...]
```

---

## Telemetry (Optional, Off by Default)

If user opts into telemetry:

```json
{
  "event": "workflow_completed",
  "workflow_type": "organizer",
  "duration_ms": 3200,
  "operations_count": 50,
  "success": true,
  "error": null
}
```

**Guarantee**: NO file contents, NO full paths, NO PII.

---

## Implementation Checklist

- [ ] Structured logging macro across all core modules
- [ ] JSON output to stderr (for debugging)
- [ ] Audit table in SQLite (append-only)
- [ ] CSV export command in Tauri
- [ ] JSON export command in Tauri
- [ ] Export functionality in UI (Download button)
- [ ] Log retention pruning (90-day window)
- [ ] Sensitive field redaction
- [ ] Test coverage (log format, export format)

---

## Monitoring & Debugging

### Local Debugging

```bash
# Follow logs in real-time
tail -f ~/.ghost/logs/current.json | jq '.'

# Filter by stage
tail -f ~/.ghost/logs/current.json | jq 'select(.stage == "execution")'

# Extract errors only
tail -f ~/.ghost/logs/current.json | jq 'select(.level == "ERROR")'
```

### Performance Analysis

```bash
# Slowest operations
jq '.[] | select(.duration_ms > 1000)' logs/*.json | \
  jq '{timestamp, action_id, duration_ms}' | \
  sort -k3 -rn

# Failure rate by operation type
jq '.[] | select(.event == "action_executed") | .type' logs/*.json | \
  sort | uniq -c | sort -rn
```

---

**Version**: 1.0  
**Status**: Ready for Phase 3.4 implementation
