# Ghost MCP Integration

Ghost should expose one Model Context Protocol (MCP) integration surface instead of separate ChatGPT, Claude, Codex, Cursor, or vendor-specific integrations.

```text
Codex / ChatGPT / Claude / Cursor / other MCP clients
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

Phase 1 should be a local stdio server launched by the installed app or CLI:

```bash
ghost mcp serve
```

Example macOS client configuration:

```json
{
  "mcpServers": {
    "ghost": {
      "command": "/Applications/Ghost.app/Contents/MacOS/ghost",
      "args": ["mcp", "serve"]
    }
  }
}
```

Example Windows client configuration:

```json
{
  "mcpServers": {
    "ghost": {
      "command": "C:\\Program Files\\Ghost\\ghost.exe",
      "args": ["mcp", "serve"]
    }
  }
}
```

Local stdio preserves Ghost's local-first posture: no Ghost cloud account, no provider upload requirement, and no network listener by default.

## Remote MCP later

Remote MCP endpoints are out of scope until local approval integrity is proven. A remote endpoint adds authentication, pairing, TLS, session expiry, replay protection, cross-origin protections, remote approval spoofing resistance, and network-exposure review.

Any future remote endpoint must be denied by default, explicitly enabled by the user, documented as higher risk, and covered by an external-agent threat model before shipping.

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
| `ghost_request_approval` | `safe-read` | Opens or focuses the Ghost desktop approval view. |
| `ghost_get_approval_status` | `safe-read` | Reports pending/approved/denied/expired. |
| `ghost_execute_approved_plan` | `local-mutate` or `os-control` | Fails unless desktop UI approved the exact plan and issued a valid token. |
| `ghost_cancel_plan` | `safe-read`/`local-mutate` | Cancels pending plan state; must not touch user files. |
| `ghost_undo_run` | `local-mutate` | Uses undo journal and policy checks; never invents reverse operations from AI output. |

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
