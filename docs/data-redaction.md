# Data Redaction

Status: **partially built**

## Shared layer

Used by (when implemented):

- OpenAI / Anthropic / local intelligence providers
- Fabric and Power BI exports
- MCP tool responses

Module: `src-tauri/src/intelligence/redaction.rs`

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

## UI requirement — **planned**

Review surfaces must show exactly what will leave the device before any remote request or export.

## Audit — **planned**

Log data categories sent, not full payloads (hashes/summaries unless verbose logging explicitly enabled).
