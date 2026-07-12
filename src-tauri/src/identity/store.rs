//! Persistence for account identity, grants, and token records.
//!
//! Encrypted at rest once a vault password is configured, via the same
//! `AuthManager::protect`/`encrypt_bytes` envelope used for workflow files —
//! and, like those files, left as plaintext until a password is set (see
//! `AuthManager::encrypt_bytes`'s doc comment). There is currently no warning
//! surfaced to the user that a linked account's tokens sit unencrypted on
//! disk in that default, no-password state.

use crate::auth::AuthManager;
use crate::identity::types::{
    AccountIdentity, IdentityBundle, IdentityProvider, IntegrationGrant, IntegrationKind,
    LegacyAccountRecord, ResourceScope, TokenMaterial, TokenRecord,
};
use std::path::PathBuf;

const BUNDLE_VERSION: u32 = 1;

#[derive(Clone)]
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

    /// Add a new integration grant (and its encrypted token record) to the
    /// existing bundle, preserving the account identity and any other
    /// grants. Errors if no identity is linked yet — a grant is meaningless
    /// without an identity to attach it to. A grant of the same
    /// `IntegrationKind` as an existing one supersedes it (re-granting
    /// replaces rather than accumulates).
    pub fn add_grant(
        &self,
        auth: &AuthManager,
        grant: IntegrationGrant,
        tokens: TokenMaterial,
    ) -> anyhow::Result<()> {
        let mut bundle = self
            .load(auth)
            .filter(|b| b.identity.is_some())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "no linked account — sign in before requesting an integration grant"
                )
            })?;

        let token_record = self.encrypt_tokens(auth, &grant.grant_id, &tokens)?;

        let superseded_grant_ids: Vec<String> = bundle
            .grants
            .iter()
            .filter(|g| g.integration == grant.integration)
            .map(|g| g.grant_id.clone())
            .collect();
        bundle.grants.retain(|g| g.integration != grant.integration);
        bundle
            .tokens
            .retain(|t| !superseded_grant_ids.contains(&t.grant_id));

        bundle.grants.push(grant);
        bundle.tokens.push(token_record);
        self.write_bundle(auth, &bundle)
    }

    /// Revoke (locally) the active grant of the given kind, if any. Sets
    /// `revoked_at` rather than deleting the record, so it stays visible in
    /// status/history views. Does not contact the provider — the token
    /// itself is not invalidated server-side.
    pub fn revoke_grant(&self, auth: &AuthManager, kind: IntegrationKind) -> anyhow::Result<()> {
        let Some(mut bundle) = self.load(auth) else {
            return Ok(());
        };
        let now = chrono::Utc::now();
        let mut changed = false;
        for grant in bundle.grants.iter_mut() {
            if grant.integration == kind && grant.is_active(now) {
                grant.revoked_at = Some(now);
                changed = true;
            }
        }
        if changed {
            self.write_bundle(auth, &bundle)?;
        }
        Ok(())
    }

    /// Decrypt and return the current access token for an active grant, if
    /// any. Returns `None` (not an error) when the grant is missing,
    /// revoked, expired, has no matching token record, or fails to decrypt
    /// — callers treat all of those as "not available right now."
    pub fn access_token_for_grant(&self, auth: &AuthManager, grant_id: &str) -> Option<String> {
        let bundle = self.load(auth)?;
        let now = chrono::Utc::now();
        let grant = bundle
            .grants
            .iter()
            .find(|g| g.grant_id == grant_id && g.is_active(now))?;
        let record = bundle
            .tokens
            .iter()
            .find(|t| t.grant_id == grant.grant_id)?;
        let plaintext = auth.decrypt_bytes(&record.access_token_encrypted).ok()?;
        String::from_utf8(plaintext).ok()
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
        // Only remove the legacy file once the new bundle is actually on
        // disk — if the write fails (e.g. disk full, permissions), leaving
        // the legacy file in place means the next launch just retries the
        // migration instead of permanently losing the linked account.
        if self.write_bundle(auth, &bundle).is_ok() {
            let _ = std::fs::remove_file(&legacy_path);
        }
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

    fn sample_grant(account_id: &str, kind: IntegrationKind) -> IntegrationGrant {
        IntegrationGrant {
            grant_id: uuid::Uuid::new_v4().to_string(),
            account_id: account_id.to_string(),
            integration: kind,
            scopes: vec!["https://analysis.windows.net/powerbi/api/.default".to_string()],
            resource_scope: ResourceScope::Global,
            granted_at: chrono::Utc::now(),
            expires_at: None,
            revoked_at: None,
        }
    }

    #[test]
    fn add_grant_preserves_existing_identity_and_grants() {
        let (store, auth, dir) = temp_store();
        let identity = sample_identity();
        store
            .store_sign_in(
                &auth,
                identity.clone(),
                &["openid", "email", "profile"],
                TokenMaterial::default(),
            )
            .unwrap();

        let grant = sample_grant(&identity.account_id, IntegrationKind::MicrosoftPowerBi);
        store
            .add_grant(
                &auth,
                grant.clone(),
                TokenMaterial {
                    access_token: "pbi-access-token".to_string(),
                    refresh_token: None,
                    expires_at: None,
                },
            )
            .unwrap();

        let loaded_identity = store.identity(&auth).unwrap();
        assert_eq!(loaded_identity.email, identity.email);

        let grants = store.active_grants(&auth);
        assert_eq!(grants.len(), 2);
        assert!(grants
            .iter()
            .any(|g| g.integration == IntegrationKind::Identity));
        assert!(grants
            .iter()
            .any(|g| g.integration == IntegrationKind::MicrosoftPowerBi));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn add_grant_without_identity_errors() {
        let (store, auth, dir) = temp_store();
        let grant = sample_grant("no-identity", IntegrationKind::MicrosoftPowerBi);
        let err = store
            .add_grant(&auth, grant, TokenMaterial::default())
            .unwrap_err();
        assert!(err.to_string().contains("sign in"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn re_granting_the_same_kind_supersedes_rather_than_accumulates() {
        let (store, auth, dir) = temp_store();
        let identity = sample_identity();
        store
            .store_sign_in(
                &auth,
                identity.clone(),
                &["openid"],
                TokenMaterial::default(),
            )
            .unwrap();

        let first = sample_grant(&identity.account_id, IntegrationKind::MicrosoftPowerBi);
        store
            .add_grant(
                &auth,
                first.clone(),
                TokenMaterial {
                    access_token: "first-token".to_string(),
                    refresh_token: None,
                    expires_at: None,
                },
            )
            .unwrap();

        let second = sample_grant(&identity.account_id, IntegrationKind::MicrosoftPowerBi);
        store
            .add_grant(
                &auth,
                second.clone(),
                TokenMaterial {
                    access_token: "second-token".to_string(),
                    refresh_token: None,
                    expires_at: None,
                },
            )
            .unwrap();

        let grants = store.active_grants(&auth);
        let pbi_grants: Vec<_> = grants
            .iter()
            .filter(|g| g.integration == IntegrationKind::MicrosoftPowerBi)
            .collect();
        assert_eq!(pbi_grants.len(), 1);
        assert_eq!(pbi_grants[0].grant_id, second.grant_id);
        assert_eq!(
            store
                .access_token_for_grant(&auth, &second.grant_id)
                .as_deref(),
            Some("second-token")
        );
        assert!(store
            .access_token_for_grant(&auth, &first.grant_id)
            .is_none());

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn access_token_for_grant_round_trips() {
        let (store, auth, dir) = temp_store();
        let identity = sample_identity();
        store
            .store_sign_in(
                &auth,
                identity.clone(),
                &["openid"],
                TokenMaterial::default(),
            )
            .unwrap();

        let grant = sample_grant(&identity.account_id, IntegrationKind::MicrosoftPowerBi);
        store
            .add_grant(
                &auth,
                grant.clone(),
                TokenMaterial {
                    access_token: "pbi-access-token".to_string(),
                    refresh_token: Some("pbi-refresh-token".to_string()),
                    expires_at: None,
                },
            )
            .unwrap();

        let token = store.access_token_for_grant(&auth, &grant.grant_id);
        assert_eq!(token.as_deref(), Some("pbi-access-token"));

        let raw = std::fs::read_to_string(store.path()).unwrap();
        assert!(!raw.contains("pbi-access-token"));
        assert!(!raw.contains("pbi-refresh-token"));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn access_token_for_grant_returns_none_when_revoked() {
        let (store, auth, dir) = temp_store();
        let identity = sample_identity();
        store
            .store_sign_in(
                &auth,
                identity.clone(),
                &["openid"],
                TokenMaterial::default(),
            )
            .unwrap();

        let grant = sample_grant(&identity.account_id, IntegrationKind::MicrosoftPowerBi);
        store
            .add_grant(
                &auth,
                grant.clone(),
                TokenMaterial {
                    access_token: "pbi-access-token".to_string(),
                    refresh_token: None,
                    expires_at: None,
                },
            )
            .unwrap();

        assert!(store
            .access_token_for_grant(&auth, &grant.grant_id)
            .is_some());

        store
            .revoke_grant(&auth, IntegrationKind::MicrosoftPowerBi)
            .unwrap();

        assert!(store
            .access_token_for_grant(&auth, &grant.grant_id)
            .is_none());
        // Revoking keeps the record visible (not deleted) — just inactive.
        let bundle_grants = store.load(&auth).unwrap().grants;
        assert!(bundle_grants
            .iter()
            .any(|g| g.grant_id == grant.grant_id && g.revoked_at.is_some()));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn revoke_grant_with_no_matching_grant_is_a_no_op() {
        let (store, auth, dir) = temp_store();
        store
            .store_sign_in(
                &auth,
                sample_identity(),
                &["openid"],
                TokenMaterial::default(),
            )
            .unwrap();
        // No Power BI grant exists yet — revoking must not error.
        store
            .revoke_grant(&auth, IntegrationKind::MicrosoftPowerBi)
            .unwrap();
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn migration_keeps_legacy_file_when_the_new_bundle_write_fails() {
        let dir =
            std::env::temp_dir().join(format!("ghost-identity-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let auth = AuthManager::with_path(dir.join("auth.json"));

        // identity.json is a pre-existing directory, not a file: atomic_write's
        // rename(tmp, identity.json) then fails on every OS (file -> directory
        // is a structural type mismatch, not a permission check root can
        // bypass), forcing write_bundle to fail deterministically.
        let identity_path = dir.join("identity.json");
        std::fs::create_dir_all(&identity_path).unwrap();
        let store = IdentityStore::with_path(identity_path);

        let legacy_path = dir.join("account.json");
        let legacy = LegacyAccountRecord {
            provider: "google".to_string(),
            email: "legacy@example.com".to_string(),
            name: "Legacy User".to_string(),
            refresh_token: Some("legacy-refresh".to_string()),
            linked_at: chrono::Utc::now(),
        };
        std::fs::write(&legacy_path, serde_json::to_string(&legacy).unwrap()).unwrap();

        let result = store.identity(&auth);

        // The in-memory migration still succeeds for this call...
        assert!(result.is_some());
        // ...but since it couldn't persist, the legacy file must survive so
        // the next launch retries instead of losing the account entirely.
        assert!(legacy_path.exists());

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
