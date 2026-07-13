//! Fabric API connector (read/export).
//!
//! Talks to the Fabric REST API for workspace/item listing and pushes audit
//! exports into a lakehouse Files folder via OneLake.

pub mod export;

use crate::identity::IntegrationError;
use serde::Deserialize;

const API_BASE: &str = "https://api.fabric.microsoft.com/v1";

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
    pub fn list_workspaces(
        &self,
        access_token: &str,
    ) -> Result<Vec<FabricWorkspace>, IntegrationError> {
        let res = self
            .http
            .get(format!("{API_BASE}/workspaces"))
            .bearer_auth(access_token)
            .send()
            .map_err(|_| IntegrationError::NetworkUnavailable)?;
        let res = map_status(res)?;
        let body: WorkspaceListResponse =
            res.json().map_err(|_| IntegrationError::InvalidResponse)?;
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
    ) -> Result<Vec<FabricLakehouse>, IntegrationError> {
        let res = self
            .http
            .get(format!("{API_BASE}/workspaces/{workspace_id}/items"))
            .bearer_auth(access_token)
            .send()
            .map_err(|_| IntegrationError::NetworkUnavailable)?;
        let res = map_status(res)?;
        let body: ItemListResponse = res.json().map_err(|_| IntegrationError::InvalidResponse)?;
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
) -> Result<reqwest::blocking::Response, IntegrationError> {
    let status = res.status();
    if status.is_success() {
        Ok(res)
    } else if status.as_u16() == 401 || status.as_u16() == 403 {
        Err(IntegrationError::ConsentRequired)
    } else {
        Err(IntegrationError::InvalidResponse)
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
