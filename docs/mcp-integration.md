# Ghost MCP Integration

Ghost should expose one Model Context Protocol (MCP) integration surface instead of separate ChatGPT, Claude, Codex, Cursor, or vendor-specific integrations.

```text
Claude Desktop / Cursor  (stock local stdio — supported now)
ChatGPT / remote clients (experimental HTTP/relay — not stock)
                         |
                         v
                 Ghost MCP Server
                         |
                         v
          Ghost planning + policy boundary
                         |
          Plan -> Review -> Approve -> Execute
                         |
                         v
              Audit log + Undo journal
```

**Client support (honest):** stock Ghost exposes local **stdio** MCP (`ghost mcp serve`). That works with Claude Desktop and Cursor via their MCP `command`/`env` config. ChatGPT does **not** use local stdio the same way; any ChatGPT path needs remote HTTP/connector support, which remains **experimental** and is not claimed as shipping in the default installer (v2.0.3). Do not market ChatGPT as ready until that transport is verified.

The MCP server is an interoperability layer. It must not become a shortcut around Ghost Guard, Zones, policy checks, desktop approval, audit logging, or undo journals.

## Product boundary

External AI clients may help a user inspect metadata, describe intent, request plans, and explain policy decisions. They must not approve their own proposals or execute unapproved mutations.

Allowed client actions:

- inspect Ghost status and non-sensitive metadata;
- scan user-approved Zones;
- suggest intent and organization rules;
- create dry-run plans;
- validate plans against policy;
- request approval in the Ghost desktop UI;
- execute only a still-valid plan that was approved in the Ghost desktop UI.

Denied client actions:

- expand a Zone or grant itself capabilities;
- approve a plan;
- modify an approved plan;
- silently overwrite, delete, upload, submit, or type;
- bypass Ghost Guard, audit logging, or undo journals;
- execute arbitrary shell commands;
- read file contents, browser data, email, document text, screenshots, or hidden files unless a future feature adds explicit scoped permission and visible state.

## Local MCP first

Phase 1 is a local stdio server launched by the installed app or CLI:

```bash
ghost mcp serve
```

Status: **built** — `src-tauri/src/mcp/server.rs` (JSON-RPC 2.0 over stdin/stdout), `handlers.rs` (read/plan/execute/undo tools), `approval.rs` (signed tokens + plan hash), `pending.rs` (approval request queue), `pairing.rs` (optional pairing gate).

### Pairing (session gate)

Pairing is enforced per **session**, not just at `initialize`: when a pairing
code is configured, `tools/list` and `tools/call` return error `-32001` until
the session has passed `initialize` with the correct code. A client that skips
`initialize` never gets tool access.

Enable it in Settings → "Connect an AI assistant (MCP)". The code can reach the
server two ways (explicit `initialize` params win over the environment):

1. **Launch environment (recommended for standard clients):** set
   `GHOST_MCP_PAIRING_CODE` in the client config's `env` block — Claude
   Desktop and Cursor can't inject custom `initialize` params, but they do
   pass `env` to the spawned server.
2. **`initialize` params:** `capabilities.ghost.pairing_code` or top-level
   `pairing_code`, for clients that support custom params.

The Settings view renders the paste-ready config below with the freshly
generated code already filled in.

Example macOS client configuration:

```json
{
  "mcpServers": {
    "ghost": {
      "command": "/Applications/Ghost.app/Contents/MacOS/Ghost",
      "args": ["mcp", "serve"],
      "env": { "GHOST_MCP_PAIRING_CODE": "ABCD2345" }
    }
  }
}
```

Example Windows client configuration:

```json
{
  "mcpServers": {
    "ghost": {
      "command": "C:\\Program Files\\Ghost\\Ghost.exe",
      "args": ["mcp", "serve"],
      "env": { "GHOST_MCP_PAIRING_CODE": "ABCD2345" }
    }
  }
}
```

With pairing disabled (the default), omit the `env` block — any local client
can connect.

Local stdio preserves Ghost's local-first posture: no Ghost cloud account, no provider upload requirement, and no network listener by default.

## Localhost HTTP (experimental v0)

```bash
ghost mcp serve http 8787
```

Binds to `127.0.0.1` only. `POST /mcp` with a JSON-RPC body — same `handle_message` path as stdio. Pairing rules still apply on `initialize`.

Settings (experimental build) can also start/stop the HTTP server from the desktop UI:

- `mcp_start_http_server` / `mcp_stop_http_server` / `mcp_http_server_status`
- Optional LAN bind (`0.0.0.0`) requires a bearer token on every `POST /mcp` request
- Optional in-process TLS via PEM `tls_cert_path` + `tls_key_path` (rustls)
- `POST /fabric/webhook` on the same listener (secret via `X-Ghost-Webhook-Secret`; configure with `fabric_set_webhook_secret`)

## Cloud relay (experimental)

Wide-area MCP without inbound firewall rules: the desktop opens an **outbound HTTPS** poll loop to a user-hosted relay (`mcp_start_relay` / `mcp_stop_relay`). Protocol and reference server: `docs/mcp-relay.md`.

## Remote MCP (opt-in LAN)

Binding beyond loopback is **denied by default** in the UI unless the user explicitly enables LAN exposure and sets a bearer token. Prefer in-process TLS cert/key paths or a reverse proxy when exposing beyond localhost.

## Tool surface

Start deliberately small and read-first.

### Phase 1: read-only

| Tool | Risk | Notes |
|---|---|---|
| `ghost_get_status` | `safe-read` | Reports app/version/capability state without user content. |
| `ghost_list_zones` | `safe-read` | Lists user-approved Zone names and metadata, not full sensitive contents. |
| `ghost_scan_zone` | `safe-read` or `sensitive-read` | Metadata-only scan by default; content reads require future scoped permission. |
| `ghost_get_run` | `safe-read` | Reads one run summary and non-sensitive audit metadata. |
| `ghost_get_audit_log` | `safe-read` | Redacted audit view by default. |

### Phase 2: planning

| Tool | Risk | Notes |
|---|---|---|
| `ghost_create_organizer_plan` | `safe-read`/`sensitive-read` planning | Creates an awaiting-review plan. Executes nothing. |
| `ghost_create_replay_plan` | `experimental`/`os-control` planning | Must stay gated until replay policy is mature. |
| `ghost_validate_plan` | `safe-read` | Re-runs policy checks against a proposed plan. |
| `ghost_explain_policy_decision` | `safe-read` | Explains denied or warned operations without weakening policy. |

### Phase 3: approved execution

| Tool | Risk | Notes |
|---|---|---|
| `ghost.request_approval` | `safe-read` | Opens or focuses the Ghost desktop approval view. |
| `ghost.get_approval_status` | `safe-read` | Reports pending/approved/denied/expired; returns `approval_token` after local approval. |
| `ghost.execute_approved_plan` | `local-mutate` | Fails unless desktop UI approved the exact Organizer plan and issued a valid token. |
| `ghost.undo_run` | `local-mutate` | Uses undo journal and policy checks; never invents reverse operations from AI output. |

### Phase 3b: saved routines (built — flagship vertical slice)

Live demo script (Claude Desktop / Cursor, stock stdio only): [`docs/claude-ghost-demo.md`](./claude-ghost-demo.md).

| Tool | Risk | Notes |
|---|---|---|
| `ghost.list_routines` | `safe-read` | Saved routine **names only** — no events, no typed text. |
| `ghost.preview_routine` | `safe-read` / `sensitive-read` (metadata) | Redacted semantic steps + policy plan; typed text always null/redacted. |
| `ghost.request_routine_approval` | `safe-read` | Creates a pending local approval bound to exact routine plan hash. |
| `ghost.get_approval_status` | `safe-read` | Same status tool; includes `kind: routine` and token when approved. |
| `ghost.execute_approved_routine` | `os-control` | Validates one-shot token + exact hash; runs canonical Action Plan runtime; returns receipt. |
| `ghost.get_run` | `safe-read` | Run summary + receipt when present. |


Desktop: Ghost polls pending approvals; for `kind: routine` it loads the routine, shows review, and `routine_issue_mcp_approval_token` issues the bound token (no auto-run).

Do not add a broad `ghost_run` tool.

## Plan result shape

Planning tools should return exact deterministic operations and policy results:

```json
{
  "plan_id": "plan_8f39",
  "status": "awaiting_review",
  "operations": [
    {
      "type": "move",
      "source": "~/Downloads/Screenshot 2026-07-11.png",
      "destination": "~/Downloads/Screenshots/2026-07/Screenshot 2026-07-11.png"
    }
  ],
  "warnings": [],
  "denied_operations": []
}
```

The model proposes intent. Ghost resolves paths, applies Zone boundaries, detects conflicts, creates deterministic operations, and routes the final plan through review.

## Approval tokens

When the user approves a plan in the Ghost desktop UI, Ghost should issue a short-lived, single-use execution token bound to the exact plan hash:

```json
{
  "plan_id": "plan_8f39",
  "plan_hash": "sha256:...",
  "approved_at": "2026-07-11T14:32:00-05:00",
  "expires_at": "2026-07-11T14:37:00-05:00",
  "capabilities": ["move", "create_directory"],
  "single_use": true
}
```

Execution must verify that:

1. the plan has not changed;
2. the plan hash matches;
3. the token has not expired;
4. the token has not already been used;
5. operations remain inside approved Zones;
6. policy still permits the operations;
7. no new conflicts appeared.

## Implementation order

1. **Partially built:** identity/grant separation, intelligence provider scaffolding, MCP tool/approval types, integration grant checks.
2. Add local read-only MCP tools (`mcp/server.rs`, stdio transport).
3. Add Organizer planning and policy-explanation tools.
4. Add desktop-approval request/status and token-gated execution (signing).
5. Add OpenAI/Anthropic/local internal intelligence providers + Settings UI.
6. Add Fabric/Power BI read + approved export.
7. Consider remote MCP only after local security and approval integrity are tested.

Rust modules: `src-tauri/src/mcp/` (scaffolding), `src-tauri/src/identity/`, `src-tauri/src/integrations/`, `src-tauri/src/intelligence/`.

## Positioning

Do not market this as vendor-specific AI being built into Ghost. Use:

```text
Use Ghost safely from the AI tools you already work with.
```

Strategic split:

- AI clients provide reasoning.
- Ghost provides controlled local execution.
- The model proposes.
- Ghost verifies.
- The user approves.
- Deterministic code acts.
