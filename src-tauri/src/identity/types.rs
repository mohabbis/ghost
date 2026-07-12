//! Layer A: account identity and integration grants.
//!
//! Signing in establishes *who* the user is. Integration grants are separate,
//! revocable records that authorize Fabric, Power BI, or other business-system
//! connectors. Identity alone must not imply access to those systems.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Identity provider used for account sign-in (Layer A only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdentityProvider {
    Microsoft,
    Google,
}

impl IdentityProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            IdentityProvider::Microsoft => "microsoft",
            IdentityProvider::Google => "google",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "microsoft" => Ok(IdentityProvider::Microsoft),
            "google" => Ok(IdentityProvider::Google),
            other => Err(format!(
                "Unknown identity provider '{other}' (expected 'microsoft' or 'google')"
            )),
        }
    }
}

/// Linked account metadata. Does not carry integration permissions or tokens.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountIdentity {
    pub account_id: String,
    pub provider: IdentityProvider,
    /// Provider subject identifier when available; otherwise a stable hash of email.
    pub subject: String,
    pub tenant_id: Option<String>,
    pub email: String,
    pub display_name: String,
    pub linked_at: DateTime<Utc>,
}

/// Business-system or base-identity integration kind (Layer B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationKind {
    /// Base sign-in scopes only (`openid`, `profile`, etc.). Not a data connector.
    Identity,
    MicrosoftFabric,
    MicrosoftPowerBi,
    GoogleCloud,
}

impl IntegrationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            IntegrationKind::Identity => "identity",
            IntegrationKind::MicrosoftFabric => "microsoft_fabric",
            IntegrationKind::MicrosoftPowerBi => "microsoft_power_bi",
            IntegrationKind::GoogleCloud => "google_cloud",
        }
    }
}

/// Narrow resource binding for an integration grant.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResourceScope {
    #[default]
    Global,
    Workspace {
        id: String,
        name: Option<String>,
    },
    Dataset {
        id: String,
        workspace_id: String,
    },
    Destination {
        id: String,
        label: Option<String>,
    },
}

/// Delegated authorization for one integration, separate from account identity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntegrationGrant {
    pub grant_id: String,
    pub account_id: String,
    pub integration: IntegrationKind,
    pub scopes: Vec<String>,
    pub resource_scope: ResourceScope,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl IntegrationGrant {
    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        if self.revoked_at.is_some() {
            return false;
        }
        if let Some(expires) = self.expires_at {
            if now >= expires {
                return false;
            }
        }
        true
    }
}

/// Encrypted token material stored separately from identity metadata.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenRecord {
    pub grant_id: String,
    pub access_token_encrypted: Vec<u8>,
    pub refresh_token_encrypted: Option<Vec<u8>>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Plaintext tokens during sign-in or refresh — never persisted or logged as-is.
#[derive(Clone, Debug, Default)]
pub struct TokenMaterial {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// On-disk bundle for one linked account (identity + grants + encrypted tokens).
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct IdentityBundle {
    pub version: u32,
    pub identity: Option<AccountIdentity>,
    pub grants: Vec<IntegrationGrant>,
    pub tokens: Vec<TokenRecord>,
}

/// Legacy single-file account shape (pre-identity refactor). Migrated on read.
#[derive(Clone, Serialize, Deserialize)]
pub struct LegacyAccountRecord {
    pub provider: String,
    pub email: String,
    pub name: String,
    pub refresh_token: Option<String>,
    pub linked_at: DateTime<Utc>,
}
