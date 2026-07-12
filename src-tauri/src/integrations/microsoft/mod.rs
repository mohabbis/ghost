//! Microsoft business-system connectors (Fabric, Power BI).
//!
//! Phase 1: module boundaries and grant checks only. API clients are planned.

pub mod fabric;
pub mod power_bi;
pub mod scopes;

use crate::auth::AuthManager;
use crate::identity::{IdentityStore, IntegrationError, IntegrationKind};

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
}
