# Approval Tokens

Status: **partially built** (claims shape + expiry helper; signing/verification **planned**)

## Purpose

MCP clients and integrations must not forge execution approval. Only Ghost's desktop UI issues valid approval artifacts after explicit user review.

## Claims shape — **built**

```rust
pub struct ApprovalTokenClaims {
    pub plan_id: String,
    pub plan_hash: String,
    pub account_id: Option<String>,
    pub approved_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub nonce: String,
}
```

Module: `src-tauri/src/mcp/approval.rs`

## Execution verification — **planned**

Before `execute_approved_plan`:

1. Plan exists and hash matches
2. Token signature valid (local signing key)
3. Token not expired
4. Token not consumed (when single-use)
5. Policy result unchanged
6. Referenced resources still match
7. No plan drift since approval

## MCP integration

See `docs/mcp-integration.md` — `ghost_execute_approved_plan` requires a valid token; clients cannot call `ghost_request_approval` and auto-accept.

## Threat scenarios

See `docs/integration-threat-model.md` — stale tokens, replay, plan drift.
