//! Fabric API connector (read/export).
//!
//! Talks to the Fabric REST API for workspace/item listing and pushes audit
//! exports into a lakehouse Files folder via OneLake.

pub mod export;

use crate::identity::IntegrationError;
use serde::Deserialize;

const API_BASE: &str = "https://api.fabric.microsoft.com/v1";

/// Which Fabric API call failed — used for clearer 403 hints.
#[derive(Clone, Copy, Debug)]
pub enum FabricOperation {
    ListWorkspaces,
    ListLakehouses,
    PushExport,
}

impl FabricOperation {
    pub fn permission_hint(self) -> &'static str {
        match self {
            FabricOperation::ListWorkspaces => {
                "Ensure your Microsoft account has access to at least one Fabric workspace and reconnect Fabric in Settings."
            }
            FabricOperation::ListLakehouses => {
                "Ensure you can view items in this workspace (Workspace Viewer or higher) and reconnect Fabric in Settings."
            }
            FabricOperation::PushExport => {
                "OneLake write access requires Contributor (or higher) on the workspace and lakehouse. If you recently changed roles, disconnect and reconnect Fabric in Settings."
            }
        }
    }
}

pub fn map_http_status(status: u16, operation: FabricOperation) -> IntegrationError {
    let _ = operation;
    match status {
        401 | 403 => IntegrationError::PermissionDenied,
        404 => IntegrationError::ResourceNotFound,
        429 => IntegrationError::RateLimited,
        code if code >= 500 => IntegrationError::ProviderUnavailable,
        _ => IntegrationError::InvalidResponse,
    }
}

/// User-facing message for a Fabric HTTP failure.
pub fn fabric_error_message(status: u16, operation: FabricOperation) -> String {
    let err = map_http_status(status, operation);
    if status == 401 || status == 403 {
        format!("{} {}", err.user_message(), operation.permission_hint())
    } else {
        err.to_string()
    }
}

pub struct FabricClient {
    http: reqwest::blocking::Client,
}

impl Default for FabricClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
pub struct FabricWorkspace {
    pub id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
pub struct FabricLakehouse {
    pub id: String,
    pub display_name: String,
    pub workspace_id: String,
}

#[derive(Deserialize)]
struct ItemListResponse {
    value: Vec<ItemEntry>,
}

#[derive(Deserialize)]
struct ItemEntry {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "type")]
    item_type: String,
}

#[derive(Deserialize)]
struct WorkspaceListResponse {
    value: Vec<WorkspaceEntry>,
}

#[derive(Deserialize)]
struct WorkspaceEntry {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
}

impl FabricClient {
    pub fn new() -> Self {
        FabricClient {
            http: reqwest::blocking::Client::new(),
        }
    }

    /// List workspaces the signed-in user can access (read-only).
    pub fn list_workspaces(&self, access_token: &str) -> Result<Vec<FabricWorkspace>, String> {
        let res = self
            .http
            .get(format!("{API_BASE}/workspaces"))
            .bearer_auth(access_token)
            .send()
            .map_err(|_| IntegrationError::NetworkUnavailable.to_string())?;
        let res = map_status(res, FabricOperation::ListWorkspaces)?;
        let body: WorkspaceListResponse = res
            .json()
            .map_err(|_| IntegrationError::InvalidResponse.to_string())?;
        Ok(body
            .value
            .into_iter()
            .map(|w| FabricWorkspace {
                id: w.id,
                display_name: w.display_name,
            })
            .collect())
    }

    /// List lakehouse items in a workspace (read-only).
    pub fn list_lakehouses(
        &self,
        access_token: &str,
        workspace_id: &str,
    ) -> Result<Vec<FabricLakehouse>, String> {
        let res = self
            .http
            .get(format!("{API_BASE}/workspaces/{workspace_id}/items"))
            .bearer_auth(access_token)
            .send()
            .map_err(|_| IntegrationError::NetworkUnavailable.to_string())?;
        let res = map_status(res, FabricOperation::ListLakehouses)?;
        let body: ItemListResponse = res
            .json()
            .map_err(|_| IntegrationError::InvalidResponse.to_string())?;
        Ok(body
            .value
            .into_iter()
            .filter(|i| i.item_type.eq_ignore_ascii_case("Lakehouse"))
            .map(|i| FabricLakehouse {
                id: i.id,
                display_name: i.display_name,
                workspace_id: workspace_id.to_string(),
            })
            .collect())
    }
}

fn map_status(
    res: reqwest::blocking::Response,
    operation: FabricOperation,
) -> Result<reqwest::blocking::Response, String> {
    let status = res.status();
    if status.is_success() {
        Ok(res)
    } else {
        Err(fabric_error_message(status.as_u16(), operation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabric_client_constructs() {
        let _ = FabricClient::new();
    }
}
