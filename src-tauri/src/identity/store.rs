//! Encrypted persistence for account identity, grants, and token records.

use crate::auth::AuthManager;
use crate::identity::types::{
    AccountIdentity, IdentityBundle, IdentityProvider, IntegrationGrant, IntegrationKind,
    LegacyAccountRecord, ResourceScope, TokenMaterial, TokenRecord,
};
use std::path::PathBuf;

const BUNDLE_VERSION: u32 = 1;

pub struct IdentityStore {
    path: PathBuf,
}

impl IdentityStore {
    pub fn new() -> Self {
        let path = dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("ghost")
            .join("identity.json");
        Self { path }
    }

    pub fn with_path(path: PathBuf) -> Self {
        IdentityStore { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Load the on-disk bundle, migrating legacy `account.json` if needed.
    pub fn load(&self, auth: &AuthManager) -> Option<IdentityBundle> {
        if let Some(bundle) = self.read_bundle(auth) {
            return Some(bundle);
        }
        self.migrate_legacy_account(auth)
    }

    pub fn identity(&self, auth: &AuthManager) -> Option<AccountIdentity> {
        self.load(auth).and_then(|b| b.identity)
    }

    pub fn active_grants(&self, auth: &AuthManager) -> Vec<IntegrationGrant> {
        let now = chrono::Utc::now();
        self.load(auth)
            .map(|b| b.grants.into_iter().filter(|g| g.is_active(now)).collect())
            .unwrap_or_default()
    }

    /// Persist a newly linked account with a base identity grant and encrypted tokens.
    pub fn store_sign_in(
        &self,
        auth: &AuthManager,
        identity: AccountIdentity,
        scopes: &[&str],
        tokens: TokenMaterial,
    ) -> anyhow::Result<()> {
        let grant_id = uuid::Uuid::new_v4().to_string();
        let grant = IntegrationGrant {
            grant_id: grant_id.clone(),
            account_id: identity.account_id.clone(),
            integration: IntegrationKind::Identity,
            scopes: scopes.iter().map(|s| (*s).to_string()).collect(),
            resource_scope: ResourceScope::Global,
            granted_at: identity.linked_at,
            expires_at: tokens.expires_at,
            revoked_at: None,
        };

        let token_record = self.encrypt_tokens(auth, &grant_id, &tokens)?;

        let bundle = IdentityBundle {
            version: BUNDLE_VERSION,
            identity: Some(identity),
            grants: vec![grant],
            tokens: vec![token_record],
        };
        self.write_bundle(auth, &bundle)
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn read_bundle(&self, auth: &AuthManager) -> Option<IdentityBundle> {
        let raw = std::fs::read_to_string(&self.path).ok()?;
        let revealed = auth.reveal(&raw).ok()?;
        serde_json::from_str(&revealed).ok()
    }

    fn write_bundle(&self, auth: &AuthManager, bundle: &IdentityBundle) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(bundle)?;
        let protected = auth.protect(&json)?;
        crate::core::security::atomic_write(&self.path, protected.as_bytes())?;
        Ok(())
    }

    fn encrypt_tokens(
        &self,
        auth: &AuthManager,
        grant_id: &str,
        tokens: &TokenMaterial,
    ) -> anyhow::Result<TokenRecord> {
        let access_enc = auth.encrypt_bytes(tokens.access_token.as_bytes())?;
        let refresh_enc = tokens
            .refresh_token
            .as_ref()
            .map(|t| auth.encrypt_bytes(t.as_bytes()))
            .transpose()?;
        Ok(TokenRecord {
            grant_id: grant_id.to_string(),
            access_token_encrypted: access_enc,
            refresh_token_encrypted: refresh_enc,
            expires_at: tokens.expires_at,
        })
    }

    /// One-time migration from legacy `account.json` (refresh token on identity record).
    fn migrate_legacy_account(&self, auth: &AuthManager) -> Option<IdentityBundle> {
        let legacy_path = self
            .path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("account.json");
        let raw = std::fs::read_to_string(&legacy_path).ok()?;
        let revealed = auth.reveal(&raw).ok()?;
        let legacy: LegacyAccountRecord = serde_json::from_str(&revealed).ok()?;
        let provider = IdentityProvider::parse(&legacy.provider).ok()?;
        let account_id = uuid::Uuid::new_v4().to_string();
        let identity = AccountIdentity {
            account_id: account_id.clone(),
            provider,
            subject: legacy.email.clone(),
            tenant_id: None,
            email: legacy.email,
            display_name: legacy.name,
            linked_at: legacy.linked_at,
        };
        let grant_id = uuid::Uuid::new_v4().to_string();
        let scopes = match provider {
            IdentityProvider::Microsoft => {
                vec![
                    "openid".to_string(),
                    "email".to_string(),
                    "profile".to_string(),
                    "offline_access".to_string(),
                ]
            }
            IdentityProvider::Google => {
                vec![
                    "openid".to_string(),
                    "email".to_string(),
                    "profile".to_string(),
                ]
            }
        };
        let grant = IntegrationGrant {
            grant_id: grant_id.clone(),
            account_id,
            integration: IntegrationKind::Identity,
            scopes,
            resource_scope: ResourceScope::Global,
            granted_at: legacy.linked_at,
            expires_at: None,
            revoked_at: None,
        };
        let tokens = TokenMaterial {
            access_token: String::new(),
            refresh_token: legacy.refresh_token,
            expires_at: None,
        };
        let token_record = self.encrypt_tokens(auth, &grant_id, &tokens).ok()?;
        let bundle = IdentityBundle {
            version: BUNDLE_VERSION,
            identity: Some(identity),
            grants: vec![grant],
            tokens: vec![token_record],
        };
        let _ = self.write_bundle(auth, &bundle);
        let _ = std::fs::remove_file(&legacy_path);
        Some(bundle)
    }
}

impl Default for IdentityStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::types::TokenMaterial;

    fn temp_store() -> (IdentityStore, AuthManager, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("ghost-identity-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        (
            IdentityStore::with_path(dir.join("identity.json")),
            AuthManager::with_path(dir.join("auth.json")),
            dir,
        )
    }

    fn sample_identity() -> AccountIdentity {
        AccountIdentity {
            account_id: uuid::Uuid::new_v4().to_string(),
            provider: IdentityProvider::Microsoft,
            subject: "user@example.com".to_string(),
            tenant_id: None,
            email: "user@example.com".to_string(),
            display_name: "Sample User".to_string(),
            linked_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn store_and_load_round_trips() {
        let (store, auth, dir) = temp_store();
        let identity = sample_identity();
        store
            .store_sign_in(
                &auth,
                identity.clone(),
                &["openid", "email", "profile"],
                TokenMaterial {
                    access_token: "access-token".to_string(),
                    refresh_token: Some("refresh-token".to_string()),
                    expires_at: None,
                },
            )
            .unwrap();

        let loaded = store.identity(&auth).unwrap();
        assert_eq!(loaded.email, identity.email);
        assert_eq!(loaded.provider, IdentityProvider::Microsoft);

        let grants = store.active_grants(&auth);
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].integration, IntegrationKind::Identity);

        let raw = std::fs::read_to_string(store.path()).unwrap();
        assert!(!raw.contains("access-token"));
        assert!(!raw.contains("refresh-token"));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn migrates_legacy_account_json() {
        let (store, auth, dir) = temp_store();
        let legacy_path = dir.join("account.json");
        let legacy = LegacyAccountRecord {
            provider: "google".to_string(),
            email: "legacy@example.com".to_string(),
            name: "Legacy User".to_string(),
            refresh_token: Some("legacy-refresh".to_string()),
            linked_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&legacy).unwrap();
        std::fs::write(&legacy_path, json).unwrap();

        let identity = store.identity(&auth).unwrap();
        assert_eq!(identity.email, "legacy@example.com");
        assert_eq!(identity.provider, IdentityProvider::Google);
        assert!(!legacy_path.exists());
        assert!(store.path().exists());

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn identity_without_fabric_grant_has_no_fabric_access() {
        let (store, auth, dir) = temp_store();
        store
            .store_sign_in(
                &auth,
                sample_identity(),
                &["openid"],
                TokenMaterial::default(),
            )
            .unwrap();

        let grants = store.active_grants(&auth);
        assert!(grants
            .iter()
            .all(|g| g.integration != IntegrationKind::MicrosoftFabric));
        assert!(grants
            .iter()
            .all(|g| g.integration != IntegrationKind::MicrosoftPowerBi));

        std::fs::remove_dir_all(dir).ok();
    }
}
