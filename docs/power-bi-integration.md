# Power BI Integration

Status: **built** (v1: grant flow, export preview, and push to "My workspace")

## Product rule

Power BI is related to Fabric but uses its own integration grant (`IntegrationKind::MicrosoftPowerBi`). Identity sign-in alone is insufficient.

## Grant requirement — **built**

`MicrosoftIntegrationService::power_bi_grant_active` requires an active Power BI grant. `request_power_bi_grant` runs incremental consent for the `power_bi::SCOPES` API scope via `identity::run_grant_flow` (reusing the same PKCE/loopback machinery as base sign-in) and persists the result as a separate `IntegrationGrant` alongside — not replacing — the base identity grant. `revoke_power_bi_grant` unlinks it locally (does not revoke provider-side consent).

Gated behind `--features experimental` (`commands/integrations.rs`) — this is a real, unproven network write to a third-party paid service, not yet part of the stock build.

## Export schema — **built (v1 approximation)**

Documented for preview/approval UI. `integrations/microsoft/power_bi/export.rs::build_export` assembles the row shapes below from Organizer execution history; every string field passes through `audit::pii::mask`. Where no source field exists yet (`estimated_time_saved_seconds`, `duration_ms`, `undo_used`), the builder sends a safe default (`0`/`false`) rather than a fabricated number — these are known gaps, not silently invented data. `workflow_id`/`workflow_name` are populated from the Organizer Zone id, since Organizer has no separate "workflow" concept (the schema predates the Organizer/Routines split).

### GhostRuns

| Column | Type |
|---|---|
| run_id | string |
| workflow_id | string |
| workflow_name | string |
| started_at | datetime |
| completed_at | datetime |
| status | string |
| approved_action_count | int |
| denied_action_count | int |
| failed_action_count | int |
| estimated_time_saved_seconds | int |
| undo_available | bool |
| undo_used | bool |

### GhostActions

| Column | Type |
|---|---|
| action_id | string |
| run_id | string |
| action_type | string |
| zone_id | string |
| risk_level | string |
| policy_result | string |
| approval_result | string |
| execution_result | string |
| duration_ms | int |
| detail | string |

`detail` (masked skip reason / failure error) is not part of the originally documented schema, added so `action_row` doesn't silently drop that information; declared in both the dataset-creation schema (`requests::dataset_definition`) and the pushed rows so they always match.

### GhostPolicyEvents

| Column | Type |
|---|---|
| event_id | string |
| run_id | string |
| policy_rule | string |
| result | string |
| reason | string |
| created_at | datetime |

Table name constants: `integrations/microsoft/power_bi/schema`.

## Default export exclusions — **built**

Every string field in the export payload passes through `audit::pii::mask` (SSN/card/email/phone patterns redacted) before it leaves `export.rs` — the same redaction `organizer_export_audit` already applies, not the separate `intelligence::redaction` module (that one is for LLM-bound Layer C planning requests, a different pipeline). There is no "verbose export" opt-in; the export always sends the masked, metadata-only payload.

## Phase-one capabilities

- ~~List workspaces, datasets/semantic models, reports~~ — **planned** (v1 has no workspace/dataset picker; it always targets "My workspace" and a single dataset named `GhostOperations`, created on first push if it doesn't exist)
- ~~Select destination dataset~~ — **planned**, same reason
- Push approved Ghost telemetry summaries — **built** (`power_bi_push_audit_export`)
- Export preview before any network write — **built** (`power_bi_export_preview`; the UI requires a preview to have been shown before the push button enables)
- Local audit of export success/failure — **planned**: the push command surfaces success/failure to the UI directly, but does not yet write a persisted local record of past export attempts
