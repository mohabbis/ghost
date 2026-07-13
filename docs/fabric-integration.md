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

See `docs/power-bi-integration.md` for the shared export payload shape.
