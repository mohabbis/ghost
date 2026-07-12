# Power BI Integration

Status: **planned** (schema + grant checks **partially built**)

## Product rule

Power BI is related to Fabric but uses its own integration grant (`IntegrationKind::MicrosoftPowerBi`). Identity sign-in alone is insufficient.

## Grant requirement — **built**

`MicrosoftIntegrationService::power_bi_grant_active` requires an active Power BI grant.

## Suggested export schema — **planned**

Documented for preview/approval UI; not yet pushed to any dataset.

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

## Default export exclusions — **planned**

No absolute paths, usernames, file contents, credentials, raw prompts, or provider responses unless user explicitly opts into verbose export.

## Phase-one capabilities — **planned**

- List workspaces, datasets/semantic models, reports
- Select destination dataset
- Push approved Ghost telemetry summaries
- Export preview before any network write
- Local audit of export success/failure
