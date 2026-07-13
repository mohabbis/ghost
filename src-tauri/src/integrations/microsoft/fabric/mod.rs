//! Fabric API connector (read/export only in phase one).
//!
//! Talks to the Fabric REST API for workspace listing and export preview.
//! Every call needs a Fabric access token from an active
//! `IntegrationKind::MicrosoftFabric` grant, never the base sign-in token.

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
