//! Microsoft business-system connectors (Fabric, Power BI).
//!
//! Phase 1: module boundaries and grant checks only. API clients are planned.

pub mod fabric;
pub mod power_bi;
pub mod scopes;

use crate::auth::AuthManager;
use crate::identity::{
    self, IdentityStore, IntegrationError, IntegrationGrant, IntegrationKind, OAuthProvider,
    ResourceScope,
};

pub struct MicrosoftIntegrationService {
    identity: IdentityStore,
}

impl MicrosoftIntegrationService {
    pub fn new(identity: IdentityStore) -> Self {
        Self { identity }
    }

    /// Fabric access requires an active `MicrosoftFabric` grant, not identity alone.
    pub fn fabric_grant_active(&self, auth: &AuthManager) -> Result<(), IntegrationError> {
        self.require_grant(auth, IntegrationKind::MicrosoftFabric)
    }

    /// Power BI access requires an active `MicrosoftPowerBi` grant.
    pub fn power_bi_grant_active(&self, auth: &AuthManager) -> Result<(), IntegrationError> {
        self.require_grant(auth, IntegrationKind::MicrosoftPowerBi)
    }

    /// Run incremental consent for Power BI (the `power_bi::SCOPES` API
    /// scope, on top of whatever identity scopes sign-in already granted)
    /// and persist the resulting grant. Requires an existing signed-in
    /// identity — base sign-in must happen first via `account_sign_in`.
    /// Blocks on browser/network interaction; run off the async runtime.
    pub fn request_power_bi_grant(
        &self,
        auth: &AuthManager,
        client_id: &str,
    ) -> Result<(), IntegrationError> {
        let account_id = self
            .identity
            .identity(auth)
            .ok_or(IntegrationError::AuthenticationRequired)?
            .account_id;

        let scope = scopes::power_bi::SCOPES.join(" ");
        let result = identity::run_grant_flow(OAuthProvider::Microsoft, client_id, &scope)
            .map_err(|_| IntegrationError::ProviderUnavailable)?;

        let grant = IntegrationGrant {
            grant_id: uuid::Uuid::new_v4().to_string(),
            account_id,
            integration: IntegrationKind::MicrosoftPowerBi,
            scopes: scopes::power_bi::SCOPES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            // v1 always targets the signed-in user's own ("My workspace")
            // Power BI workspace — no workspace id to scope to yet.
            resource_scope: ResourceScope::Global,
            granted_at: chrono::Utc::now(),
            expires_at: result.tokens.expires_at,
            revoked_at: None,
        };

        self.identity
            .add_grant(auth, grant, result.tokens)
            .map_err(|_| IntegrationError::TokenRefreshFailed)
    }

    /// Revoke the local Power BI grant (does not contact Microsoft).
    pub fn revoke_power_bi_grant(&self, auth: &AuthManager) -> Result<(), IntegrationError> {
        self.identity
            .revoke_grant(auth, IntegrationKind::MicrosoftPowerBi)
            .map_err(|_| IntegrationError::TokenRefreshFailed)
    }

    /// Decrypted access token for the active Power BI grant, if any.
    pub fn power_bi_access_token(&self, auth: &AuthManager) -> Result<String, IntegrationError> {
        self.power_bi_grant_active(auth)?;
        let grant_id = self
            .identity
            .active_grants(auth)
            .into_iter()
            .find(|g| g.integration == IntegrationKind::MicrosoftPowerBi)
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
        let active = self.identity.active_grants(auth);
        if active.iter().any(|g| g.integration == kind) {
            Ok(())
        } else {
            Err(IntegrationError::ConsentRequired)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{AccountIdentity, IdentityProvider, IdentityStore, TokenMaterial};

    #[test]
    fn identity_alone_cannot_access_fabric() {
        let dir = std::env::temp_dir().join(format!("ghost-ms-int-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let identity_path = dir.join("identity.json");
        let store = IdentityStore::with_path(identity_path.clone());
        let auth = AuthManager::with_path(dir.join("auth.json"));
        let svc = MicrosoftIntegrationService::new(IdentityStore::with_path(identity_path));

        store
            .store_sign_in(
                &auth,
                AccountIdentity {
                    account_id: uuid::Uuid::new_v4().to_string(),
                    provider: IdentityProvider::Microsoft,
                    subject: "sub".to_string(),
                    tenant_id: None,
                    email: "user@example.com".to_string(),
                    display_name: "User".to_string(),
                    linked_at: chrono::Utc::now(),
                },
                &["openid"],
                TokenMaterial::default(),
            )
            .unwrap();

        assert!(matches!(
            svc.fabric_grant_active(&auth),
            Err(IntegrationError::ConsentRequired)
        ));
        std::fs::remove_dir_all(dir).ok();
    }

    /// Exercises the grant persistence + access-token path directly via
    /// `IdentityStore::add_grant` rather than the interactive OAuth flow
    /// (`request_power_bi_grant` needs a real browser + network round trip,
    /// which this environment can't perform) — mirrors how `store_sign_in`
    /// is already tested directly elsewhere.
    #[test]
    fn power_bi_grant_persists_and_is_active_after_add_grant() {
        let dir = std::env::temp_dir().join(format!("ghost-ms-int-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let identity_path = dir.join("identity.json");
        let store = IdentityStore::with_path(identity_path.clone());
        let auth = AuthManager::with_path(dir.join("auth.json"));
        let svc = MicrosoftIntegrationService::new(IdentityStore::with_path(identity_path));

        let account_id = uuid::Uuid::new_v4().to_string();
        store
            .store_sign_in(
                &auth,
                AccountIdentity {
                    account_id: account_id.clone(),
                    provider: IdentityProvider::Microsoft,
                    subject: "sub".to_string(),
                    tenant_id: None,
                    email: "user@example.com".to_string(),
                    display_name: "User".to_string(),
                    linked_at: chrono::Utc::now(),
                },
                &["openid"],
                TokenMaterial::default(),
            )
            .unwrap();

        assert!(matches!(
            svc.power_bi_grant_active(&auth),
            Err(IntegrationError::ConsentRequired)
        ));

        let grant = IntegrationGrant {
            grant_id: uuid::Uuid::new_v4().to_string(),
            account_id,
            integration: IntegrationKind::MicrosoftPowerBi,
            scopes: crate::integrations::microsoft::scopes::power_bi::SCOPES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            resource_scope: ResourceScope::Global,
            granted_at: chrono::Utc::now(),
            expires_at: None,
            revoked_at: None,
        };
        store
            .add_grant(
                &auth,
                grant,
                TokenMaterial {
                    access_token: "pbi-access-token".to_string(),
                    refresh_token: None,
                    expires_at: None,
                },
            )
            .unwrap();

        assert!(svc.power_bi_grant_active(&auth).is_ok());
        assert_eq!(
            svc.power_bi_access_token(&auth).as_deref(),
            Ok("pbi-access-token")
        );

        svc.revoke_power_bi_grant(&auth).unwrap();
        assert!(matches!(
            svc.power_bi_grant_active(&auth),
            Err(IntegrationError::ConsentRequired)
        ));
        assert!(matches!(
            svc.power_bi_access_token(&auth),
            Err(IntegrationError::ConsentRequired)
        ));

        std::fs::remove_dir_all(dir).ok();
    }
}
