# Fabric Integration

Status: **partially built** (grant flow, workspace list, export preview — experimental)

## Product rule

Fabric is a **business-system connector** (Layer B), not an intelligence provider. Fabric pipelines must not trigger desktop mutations without Ghost's full approval pipeline.

## First use case

```text
Ghost audit history
  -> user selects date range
  -> structured summary
  -> user reviews export preview
  -> user approves (push to Fabric destination — planned)
  -> local audit entry
```

v1 ships: grant, workspace list, lakehouse list, export preview, and **push** to a lakehouse `Files/ghost-export/` folder via OneLake (experimental).

## Module layout — **built**

```text
src-tauri/src/integrations/microsoft/
├── fabric/
│   └── mod.rs        FabricClient::list_workspaces
├── scopes.rs         fabric::SCOPES = api.fabric.microsoft.com/.default
└── mod.rs            grant flow + fabric_access_token
```

Commands (`commands/integrations.rs`, experimental):

| Command | Risk | Notes |
|---|---|---|
| `fabric_grant_status` | `safe-read` | Local grant metadata |
| `fabric_request_grant` | `external-mutate` | Incremental OAuth consent |
| `fabric_revoke_grant` | `local-mutate` | Local revoke only |
| `fabric_list_workspaces` | `safe-read` | Requires active Fabric grant |
| `fabric_export_preview` | `safe-read` | Reuses `power_bi/export.rs` row shapes |
| `fabric_list_lakehouses` | `safe-read` | Lists lakehouse items in a workspace |
| `fabric_push_audit_export` | `external-mutate` | Uploads JSON export files to OneLake Files (preview first) |

## Grant requirement — **built**

`MicrosoftIntegrationService::fabric_grant_active` returns `ConsentRequired` when only an identity grant exists. `request_fabric_grant` persists a separate `IntegrationKind::MicrosoftFabric` grant.

## Not in scope (v1)

- **Auto-executing** inbound Fabric-triggered file mutations
- Notebook/pipeline direct desktop control
- Silent export

## Inbound intents (experimental, read-only queue)

`fabric_record_inbound_intent` stores a pending intent locally (webhook simulation / manual registration). `fabric_list_inbound_intents` and Organizer UI banners surface it — the user must still scan, review, and approve in Organizer. `fabric_dismiss_inbound_intent` clears without executing.

### Webhook ingestion (experimental)

When the MCP HTTP server is running (`mcp_start_http_server`), Fabric or Eventstream connectors can POST to `/fabric/webhook`:

```http
POST /fabric/webhook HTTP/1.1
X-Ghost-Webhook-Secret: <secret from fabric_set_webhook_secret>
Content-Type: application/json

{"zone_id":"optional-zone-id","source":"fabric-pipeline","summary":"Pipeline completed — review exports"}
```

Ghost records the intent via `triggers::record_intent` and returns the intent JSON. No filesystem or replay mutation occurs.

Commands: `fabric_set_webhook_secret`, `fabric_webhook_status`.

See `docs/power-bi-integration.md` for the shared export payload shape.
