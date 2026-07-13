# Approval Tokens

Status: **built** (signing, verification, plan-hash binding, single-use nonces)

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

## Issuing tokens — **built**

`organizer_issue_mcp_approval_token` (stable) hashes the current server-side plan via `mcp/plan_hash.rs` and returns a signed JSON token (~5 minute TTL). The Organizer UI exposes **MCP token** after the user scans and reviews a plan.

## Execution verification — **built**

Before `ghost.execute_approved_plan`:

1. Re-plan server-side and hash must match `claims.plan_hash`
2. Token signature valid (local signing key at `data_dir/ghost/mcp-signing.key`)
3. Token not expired
4. Token nonce not already consumed (`mcp/token_store.rs`)
5. Zone id matches `plan_id`
6. Plan has no denied operations

MCP execution calls `organizer/pipeline.rs::execute_zone` — the same path as `organizer_execute`.

## MCP integration

See `docs/mcp-integration.md` — `ghost.execute_approved_plan` requires a valid token; clients cannot approve plans themselves.

## Threat scenarios

See `docs/integration-threat-model.md` — stale tokens, replay, plan drift.
