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

## Pending approval requests — **built**

`ghost.request_approval` writes a file-backed pending request (`mcp/pending.rs`). The desktop app polls `mcp_list_pending_approvals` and focuses Organizer when a new request arrives. `organizer_issue_mcp_approval_token` marks the matching request **approved**; MCP clients poll `ghost.get_approval_status` until approved, then the user supplies the issued token.

MCP `ghost.undo_run` uses the undo journal only — no approval token is required (reversible by design).

## MCP integration

See `docs/mcp-integration.md` — `ghost.execute_approved_plan` requires a valid token; clients cannot approve plans themselves.

## Threat scenarios

See `docs/integration-threat-model.md` — stale tokens, replay, plan drift.
