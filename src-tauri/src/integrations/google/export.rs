//! Push audit export payloads into a GCS bucket as JSON objects.

use crate::identity::IntegrationError;
use crate::integrations::google::google_error_message;
use crate::integrations::microsoft::power_bi::export::AuditExportPayload;
use serde_json::Value;

pub struct GcsExportClient {
    http: reqwest::blocking::Client,
}

impl Default for GcsExportClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GcsExportClient {
    pub fn new() -> Self {
        GcsExportClient {
            http: reqwest::blocking::Client::new(),
        }
    }

    pub fn push_audit_export(
        &self,
        access_token: &str,
        bucket: &str,
        payload: &AuditExportPayload,
    ) -> Result<GcsPushSummary, String> {
        let prefix = format!(
            "ghost-export/{}",
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
        );
        self.upload_json(
            access_token,
            bucket,
            &format!("{prefix}/runs.json"),
            &payload.runs,
        )?;
        self.upload_json(
            access_token,
            bucket,
            &format!("{prefix}/actions.json"),
            &payload.actions,
        )?;
        self.upload_json(
            access_token,
            bucket,
            &format!("{prefix}/policy_events.json"),
            &payload.policy_events,
        )?;
        Ok(GcsPushSummary {
            runs_pushed: payload.runs.len(),
            actions_pushed: payload.actions.len(),
            policy_events_pushed: payload.policy_events.len(),
            export_prefix: prefix,
        })
    }

    fn upload_json(
        &self,
        access_token: &str,
        bucket: &str,
        object_name: &str,
        rows: &[Value],
    ) -> Result<(), String> {
        let body =
            serde_json::to_vec(rows).map_err(|_| IntegrationError::InvalidResponse.to_string())?;
        let res = self
            .http
            .post(format!(
                "https://storage.googleapis.com/upload/storage/v1/b/{bucket}/o"
            ))
            .query(&[("uploadType", "media"), ("name", object_name)])
            .header("Content-Type", "application/json")
            .bearer_auth(access_token)
            .body(body)
            .send()
            .map_err(|_| IntegrationError::NetworkUnavailable.to_string())?;
        let status = res.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(google_error_message(status.as_u16(), "upload"))
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct GcsPushSummary {
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
        let _ = GcsExportClient::new();
    }
}
