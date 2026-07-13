//! Power BI REST connector (read/export only in phase one).
//!
//! Talks to the Power BI Push Datasets API
//! (<https://learn.microsoft.com/en-us/rest/api/power-bi/push-datasets>):
//! ensure a dataset with the `docs/power-bi-integration.md` schema exists in
//! the signed-in user's own workspace ("My workspace" — v1 never targets a
//! shared workspace), then push rows into it. Every call needs a Power BI
//! access token from an active `IntegrationKind::MicrosoftPowerBi` grant
//! (`integrations::microsoft::MicrosoftIntegrationService`), never the base
//! sign-in token.

pub mod export;

use crate::identity::IntegrationError;
use serde::Deserialize;
use serde_json::Value;

const API_BASE: &str = "https://api.powerbi.com/v1.0/myorg";

pub struct PowerBiClient {
    http: reqwest::blocking::Client,
}

impl Default for PowerBiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct DatasetListResponse {
    value: Vec<DatasetSummary>,
}

#[derive(Deserialize)]
struct DatasetSummary {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct CreatedDataset {
    id: String,
}

impl PowerBiClient {
    pub fn new() -> Self {
        PowerBiClient {
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Return the id of an existing push dataset named `dataset_name` in "My
    /// workspace", creating it with the `requests::dataset_definition` schema
    /// if none exists yet. Never creates a second dataset with the same name.
    pub fn ensure_dataset(
        &self,
        access_token: &str,
        dataset_name: &str,
    ) -> Result<String, IntegrationError> {
        let list_res = self
            .http
            .get(format!("{API_BASE}/datasets"))
            .bearer_auth(access_token)
            .send()
            .map_err(|_| IntegrationError::NetworkUnavailable)?;
        let list_res = map_status(list_res)?;
        let list: DatasetListResponse = list_res
            .json()
            .map_err(|_| IntegrationError::InvalidResponse)?;
        if let Some(existing) = list.value.into_iter().find(|d| d.name == dataset_name) {
            return Ok(existing.id);
        }

        let create_res = self
            .http
            .post(format!("{API_BASE}/datasets"))
            .bearer_auth(access_token)
            .json(&requests::dataset_definition(dataset_name))
            .send()
            .map_err(|_| IntegrationError::NetworkUnavailable)?;
        let create_res = map_status(create_res)?;
        let created: CreatedDataset = create_res
            .json()
            .map_err(|_| IntegrationError::InvalidResponse)?;
        Ok(created.id)
    }

    /// Push rows into one table of an existing dataset.
    pub fn push_rows(
        &self,
        access_token: &str,
        dataset_id: &str,
        table: &str,
        rows: &[Value],
    ) -> Result<(), IntegrationError> {
        if rows.is_empty() {
            return Ok(());
        }
        let res = self
            .http
            .post(format!(
                "{API_BASE}/datasets/{dataset_id}/tables/{table}/rows"
            ))
            .bearer_auth(access_token)
            .json(&requests::rows_body(rows))
            .send()
            .map_err(|_| IntegrationError::NetworkUnavailable)?;
        map_status(res)?;
        Ok(())
    }
}

/// Map an HTTP response's status to `IntegrationError`, consuming (and
/// discarding) the body on failure so the caller never has to.
fn map_status(
    res: reqwest::blocking::Response,
) -> Result<reqwest::blocking::Response, IntegrationError> {
    match res.status().as_u16() {
        200..=299 => Ok(res),
        401 => Err(IntegrationError::TokenExpired),
        403 => Err(IntegrationError::PermissionDenied),
        404 => Err(IntegrationError::ResourceNotFound),
        429 => Err(IntegrationError::RateLimited),
        500..=599 => Err(IntegrationError::ProviderUnavailable),
        _ => Err(IntegrationError::InvalidResponse),
    }
}

/// Suggested schema for Ghost operations dashboards (export preview only).
pub mod schema {
    pub const GHOST_RUNS_TABLE: &str = "GhostRuns";
    pub const GHOST_ACTIONS_TABLE: &str = "GhostActions";
    pub const GHOST_POLICY_EVENTS_TABLE: &str = "GhostPolicyEvents";
}

/// Pure, network-free JSON-body builders — unit-testable without a live
/// server. Mirrors the Power BI Push Datasets API's documented request
/// shapes exactly, so the only thing untested end to end is the network
/// call itself (see `docs/integrations-roadmap.md` for why this environment
/// can't exercise that against a real tenant).
pub mod requests {
    use super::schema::{GHOST_ACTIONS_TABLE, GHOST_POLICY_EVENTS_TABLE, GHOST_RUNS_TABLE};
    use serde_json::{json, Value};

    fn column(name: &str, data_type: &str) -> Value {
        json!({ "name": name, "dataType": data_type })
    }

    /// The dataset-creation body: one push dataset named `dataset_name` with
    /// the three tables from `docs/power-bi-integration.md`. Column order
    /// matches the doc; `action_row`/`policy_event_row` in `export.rs` add
    /// one extra `detail`/free-text column beyond the documented set to
    /// avoid silently dropping skip/failure reasons — declared here too so
    /// pushed rows always match the dataset's own schema.
    pub fn dataset_definition(dataset_name: &str) -> Value {
        json!({
            "name": dataset_name,
            "defaultMode": "Push",
            "tables": [
                {
                    "name": GHOST_RUNS_TABLE,
                    "columns": [
                        column("run_id", "String"),
                        column("workflow_id", "String"),
                        column("workflow_name", "String"),
                        column("started_at", "DateTime"),
                        column("completed_at", "DateTime"),
                        column("status", "String"),
                        column("approved_action_count", "Int64"),
                        column("denied_action_count", "Int64"),
                        column("failed_action_count", "Int64"),
                        column("estimated_time_saved_seconds", "Int64"),
                        column("undo_available", "Boolean"),
                        column("undo_used", "Boolean"),
                    ],
                },
                {
                    "name": GHOST_ACTIONS_TABLE,
                    "columns": [
                        column("action_id", "String"),
                        column("run_id", "String"),
                        column("action_type", "String"),
                        column("zone_id", "String"),
                        column("risk_level", "String"),
                        column("policy_result", "String"),
                        column("approval_result", "String"),
                        column("execution_result", "String"),
                        column("duration_ms", "Int64"),
                        column("detail", "String"),
                    ],
                },
                {
                    "name": GHOST_POLICY_EVENTS_TABLE,
                    "columns": [
                        column("event_id", "String"),
                        column("run_id", "String"),
                        column("policy_rule", "String"),
                        column("result", "String"),
                        column("reason", "String"),
                        column("created_at", "DateTime"),
                    ],
                },
            ],
        })
    }

    /// The row-push body: `{"rows": [...]}`, per the Push Datasets API.
    pub fn rows_body(rows: &[Value]) -> Value {
        json!({ "rows": rows })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn dataset_definition_declares_all_three_tables_with_expected_names() {
            let def = dataset_definition("GhostOps");
            assert_eq!(def["name"], "GhostOps");
            assert_eq!(def["defaultMode"], "Push");
            let tables = def["tables"].as_array().unwrap();
            let names: Vec<&str> = tables.iter().map(|t| t["name"].as_str().unwrap()).collect();
            assert_eq!(
                names,
                vec![
                    GHOST_RUNS_TABLE,
                    GHOST_ACTIONS_TABLE,
                    GHOST_POLICY_EVENTS_TABLE
                ]
            );
        }

        #[test]
        fn ghost_runs_table_columns_match_documented_schema() {
            let def = dataset_definition("GhostOps");
            let runs = def["tables"][0]["columns"].as_array().unwrap();
            let names: Vec<&str> = runs.iter().map(|c| c["name"].as_str().unwrap()).collect();
            assert_eq!(
                names,
                vec![
                    "run_id",
                    "workflow_id",
                    "workflow_name",
                    "started_at",
                    "completed_at",
                    "status",
                    "approved_action_count",
                    "denied_action_count",
                    "failed_action_count",
                    "estimated_time_saved_seconds",
                    "undo_available",
                    "undo_used",
                ]
            );
        }

        #[test]
        fn rows_body_wraps_rows_under_the_rows_key() {
            let rows = vec![json!({"a": 1}), json!({"a": 2})];
            let body = rows_body(&rows);
            assert_eq!(body["rows"].as_array().unwrap().len(), 2);
            assert_eq!(body["rows"][0]["a"], 1);
        }
    }
}
