# Data Redaction

Status: **partially built**

## Shared layer

There are two separate redaction pipelines for two separate data paths — not
one shared module used everywhere, despite this doc's original framing:

- **`src-tauri/src/intelligence/redaction.rs`** (`DataSensitivity`,
  `redact_planning_request`) — for Layer C, the LLM-bound path: OpenAI /
  Anthropic / local intelligence providers' `PlanningRequest` payloads.
- **`src-tauri/src/audit/pii.rs`** (`mask`) — for Layer B business-system
  exports: pattern-based (SSN/card/email/phone) redaction of any string
  field. This is what `organizer_export_audit` already used, and what
  Power BI export (`integrations/microsoft/power_bi/export.rs::build_export`)
  now uses too — **not** `intelligence::redaction`, which has no reason to
  touch Organizer execution history. MCP tool responses, if built, would
  need their own decision between the two (or a third), not an assumption
  that one pipeline already covers them.

## Classification — **built**

```rust
pub enum DataSensitivity {
    Public,
    InternalMetadata,
    Personal,
    Confidential,
    Secret,
}
```

## Organizer planning default — **built**

Send only:

```json
{
  "name": "invoice-july.pdf",
  "extension": "pdf",
  "size": 84521,
  "created_at": "2026-07-08",
  "zone_relative_path": "invoice-july.pdf"
}
```

## Excluded by default

- File contents and document text
- Absolute paths and usernames
- Credentials, tokens, secrets
- Browser/email/screen data
- Raw model prompts and full provider responses
- Hidden files

## UI requirement

Review surfaces must show exactly what will leave the device before any remote request or export. **Built for Power BI export** (`power_bi_export_preview`; the Settings UI requires a preview before the push button enables) — **planned** for intelligence-provider requests and any future Fabric/MCP export.

## Audit — **planned**

Log data categories sent, not full payloads (hashes/summaries unless verbose logging explicitly enabled).
