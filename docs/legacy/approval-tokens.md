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

- `organizer_issue_mcp_approval_token` (stable) hashes the current server-side Organizer plan via `mcp/plan_hash.rs` and returns a signed JSON token (~5 minute TTL). The Organizer UI exposes **MCP token** after the user scans and reviews a plan.
- `routine_issue_mcp_approval_token` (stable) reloads a saved routine, re-hashes with `hash_routine_plan`, refuses policy `Deny`, and marks the exact pending MCP request approved. The Record & Verify UI surfaces **Approve for MCP** when a routine request arrives.

## Execution verification — **built**

Before `ghost.execute_approved_plan` / `ghost.execute_approved_routine`:

1. Re-plan / reload server-side and hash must match `claims.plan_hash`
2. Token signature valid (local signing key at `data_dir/ghost/mcp-signing.key`)
3. Token not expired
4. Token nonce not already consumed (`mcp/token_store.rs`)
5. `plan_id` matches (`plan_{zone}` or `routine:{name}`)
6. Plan has no denied operations

Organizer MCP execution uses the canonical Action Plan runtime (`PlanSource::Mcp`). Routine MCP execution uses the same `run_persisted_action_plan` path with a compiled routine Action Plan (engine for UI steps).

## Pending approval requests — **built**

`ghost.request_approval` / `ghost.request_routine_approval` write file-backed pending requests (`mcp/pending.rs`, with `kind: organizer|routine`). The desktop app polls `mcp_list_pending_approvals` and focuses Organizer or Record & Verify. After local approval the signed token is stored on the request; MCP clients poll `ghost.get_approval_status` until `approved` and read `approval_token`.

MCP `ghost.undo_run` uses the undo journal only — no approval token is required (reversible by design for filesystem undos).

## MCP integration

See `docs/mcp-integration.md` — `ghost.execute_approved_plan` requires a valid token; clients cannot approve plans themselves.

## Threat scenarios

See `docs/integration-threat-model.md` — stale tokens, replay, plan drift.
