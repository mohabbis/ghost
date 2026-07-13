//! Push audit export payloads into a Fabric lakehouse via OneLake Files API.

use crate::identity::IntegrationError;
use crate::integrations::microsoft::power_bi::export::AuditExportPayload;
use serde_json::Value;

const ONELAKE_DFS: &str = "https://onelake.dfs.fabric.microsoft.com";

pub struct FabricExportClient {
    http: reqwest::blocking::Client,
}

impl Default for FabricExportClient {
    fn default() -> Self {
        Self::new()
    }
}

impl FabricExportClient {
    pub fn new() -> Self {
        FabricExportClient {
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Upload the three export tables as JSON files under `Files/ghost-export/`.
    pub fn push_audit_export(
        &self,
        access_token: &str,
        workspace_id: &str,
        lakehouse_id: &str,
        payload: &AuditExportPayload,
    ) -> Result<FabricPushSummary, IntegrationError> {
        let prefix = format!(
            "ghost-export/{}",
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
        );
        let runs_path = format!("{prefix}/runs.json");
        let actions_path = format!("{prefix}/actions.json");
        let policy_path = format!("{prefix}/policy_events.json");

        self.upload_json(
            access_token,
            workspace_id,
            lakehouse_id,
            &runs_path,
            &payload.runs,
        )?;
        self.upload_json(
            access_token,
            workspace_id,
            lakehouse_id,
            &actions_path,
            &payload.actions,
        )?;
        self.upload_json(
            access_token,
            workspace_id,
            lakehouse_id,
            &policy_path,
            &payload.policy_events,
        )?;

        Ok(FabricPushSummary {
            runs_pushed: payload.runs.len(),
            actions_pushed: payload.actions.len(),
            policy_events_pushed: payload.policy_events.len(),
            export_prefix: prefix,
        })
    }

    fn upload_json(
        &self,
        access_token: &str,
        workspace_id: &str,
        lakehouse_id: &str,
        relative_path: &str,
        rows: &[Value],
    ) -> Result<(), IntegrationError> {
        let body = serde_json::to_vec(rows).map_err(|_| IntegrationError::InvalidResponse)?;
        let url = format!(
            "{ONELAKE_DFS}/{workspace_id}/{lakehouse_id}.Lakehouse/Files/{relative_path}?resource=file"
        );
        let res = self
            .http
            .put(&url)
            .header("x-ms-version", "2021-06-08")
            .header("Content-Type", "application/json")
            .header("Content-Length", body.len())
            .bearer_auth(access_token)
            .body(body)
            .send()
            .map_err(|_| IntegrationError::NetworkUnavailable)?;
        let status = res.status();
        if status.is_success() || status.as_u16() == 201 {
            Ok(())
        } else if status.as_u16() == 401 || status.as_u16() == 403 {
            Err(IntegrationError::ConsentRequired)
        } else {
            Err(IntegrationError::InvalidResponse)
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct FabricPushSummary {
    pub runs_pushed: usize,
    pub actions_pushed: usize,
    pub policy_events_pushed: usize,
    pub export_prefix: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_client_constructs() {
        let _ = FabricExportClient::new();
    }
}
