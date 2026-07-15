//! Google Cloud Storage connector (read/export).

pub mod export;
pub mod scopes;

use crate::auth::AuthManager;
use crate::identity::{
    self, IdentityStore, IntegrationError, IntegrationGrant, IntegrationKind, OAuthProvider,
    ResourceScope,
};
use serde::Deserialize;

const STORAGE_API: &str = "https://storage.googleapis.com/storage/v1";

pub struct GoogleIntegrationService {
    identity: IdentityStore,
}

impl GoogleIntegrationService {
    pub fn new(identity: IdentityStore) -> Self {
        Self { identity }
    }

    pub fn google_grant_active(&self, auth: &AuthManager) -> Result<(), IntegrationError> {
        self.require_grant(auth, IntegrationKind::GoogleCloud)
    }

    pub fn request_google_grant(
        &self,
        auth: &AuthManager,
        client_id: &str,
    ) -> Result<(), IntegrationError> {
        let account_id = self
            .identity
            .identity(auth)
            .ok_or(IntegrationError::AuthenticationRequired)?
            .account_id;

        let scope = scopes::storage::SCOPES.join(" ");
        let result = identity::run_grant_flow(OAuthProvider::Google, client_id, &scope)
            .map_err(|_| IntegrationError::ProviderUnavailable)?;

        let grant = IntegrationGrant {
            grant_id: uuid::Uuid::new_v4().to_string(),
            account_id,
            integration: IntegrationKind::GoogleCloud,
            scopes: scopes::storage::SCOPES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            resource_scope: ResourceScope::Global,
            granted_at: chrono::Utc::now(),
            expires_at: result.tokens.expires_at,
            revoked_at: None,
        };

        self.identity
            .add_grant(auth, grant, result.tokens)
            .map_err(|_| IntegrationError::TokenRefreshFailed)
    }

    pub fn revoke_google_grant(&self, auth: &AuthManager) -> Result<(), IntegrationError> {
        self.identity
            .revoke_grant(auth, IntegrationKind::GoogleCloud)
            .map_err(|_| IntegrationError::TokenRefreshFailed)
    }

    /// Narrow the active Google Cloud grant to a single export bucket.
    pub fn bind_export_bucket(
        &self,
        auth: &AuthManager,
        bucket: &str,
    ) -> Result<(), IntegrationError> {
        let bucket = bucket.trim();
        if bucket.is_empty() {
            return Err(IntegrationError::InvalidResponse);
        }
        self.google_grant_active(auth)?;
        let scope = ResourceScope::Destination {
            id: bucket.to_string(),
            label: None,
        };
        self.identity
            .set_grant_resource_scope(auth, IntegrationKind::GoogleCloud, scope)
            .map_err(|_| IntegrationError::TokenRefreshFailed)
    }

    /// Bound export bucket when the grant is scoped to `Destination`.
    pub fn bound_export_bucket(&self, auth: &AuthManager) -> Option<String> {
        self.identity
            .active_grant(auth, IntegrationKind::GoogleCloud)
            .and_then(|g| match g.resource_scope {
                ResourceScope::Destination { id, .. } => Some(id),
                ResourceScope::Global => None,
                _ => None,
            })
    }

    /// Ensure push targets match the bound bucket when scope is narrowed.
    pub fn ensure_push_bucket_allowed(
        &self,
        auth: &AuthManager,
        bucket: &str,
    ) -> Result<(), IntegrationError> {
        self.google_grant_active(auth)?;
        if let Some(bound) = self.bound_export_bucket(auth)
            && bound != bucket
        {
            return Err(IntegrationError::PermissionDenied);
        }
        Ok(())
    }

    pub fn google_access_token(&self, auth: &AuthManager) -> Result<String, IntegrationError> {
        self.google_grant_active(auth)?;
        let grant_id = self
            .identity
            .active_grants(auth)
            .into_iter()
            .find(|g| g.integration == IntegrationKind::GoogleCloud)
            .map(|g| g.grant_id)
            .ok_or(IntegrationError::ConsentRequired)?;
        self.identity
            .access_token_for_grant(auth, &grant_id)
            .ok_or(IntegrationError::TokenExpired)
    }

    fn require_grant(
        &self,
        auth: &AuthManager,
        kind: IntegrationKind,
    ) -> Result<(), IntegrationError> {
        if self.identity.identity(auth).is_none() {
            return Err(IntegrationError::AuthenticationRequired);
        }
        if self
            .identity
            .active_grants(auth)
            .iter()
            .any(|g| g.integration == kind)
        {
            Ok(())
        } else {
            Err(IntegrationError::ConsentRequired)
        }
    }
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
pub struct GcsBucket {
    pub name: String,
    pub location: Option<String>,
}

#[derive(Deserialize)]
struct BucketListResponse {
    items: Option<Vec<BucketEntry>>,
}

#[derive(Deserialize)]
struct BucketEntry {
    name: String,
    location: Option<String>,
}

pub struct GcsClient {
    http: reqwest::blocking::Client,
}

impl Default for GcsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GcsClient {
    pub fn new() -> Self {
        GcsClient {
            http: reqwest::blocking::Client::new(),
        }
    }

    pub fn list_buckets(
        &self,
        access_token: &str,
        project_id: &str,
    ) -> Result<Vec<GcsBucket>, String> {
        let res = self
            .http
            .get(format!("{STORAGE_API}/b"))
            .query(&[("project", project_id)])
            .bearer_auth(access_token)
            .send()
            .map_err(|_| IntegrationError::NetworkUnavailable.to_string())?;
        let status = res.status();
        if !status.is_success() {
            return Err(google_error_message(status.as_u16(), "list buckets"));
        }
        let body: BucketListResponse = res
            .json()
            .map_err(|_| IntegrationError::InvalidResponse.to_string())?;
        Ok(body
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|b| GcsBucket {
                name: b.name,
                location: b.location,
            })
            .collect())
    }
}

pub fn google_error_message(status: u16, operation: &str) -> String {
    match status {
        401 | 403 => format!(
            "{} For Google Cloud Storage {operation}, ensure the account has Storage Object User (or higher) on the target bucket and reconnect in Settings.",
            IntegrationError::PermissionDenied.user_message()
        ),
        404 => IntegrationError::ResourceNotFound.to_string(),
        429 => IntegrationError::RateLimited.to_string(),
        code if code >= 500 => IntegrationError::ProviderUnavailable.to_string(),
        _ => IntegrationError::InvalidResponse.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcs_client_constructs() {
        let _ = GcsClient::new();
    }
}
