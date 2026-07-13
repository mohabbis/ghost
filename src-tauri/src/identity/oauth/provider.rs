use crate::identity::types::IdentityProvider;

/// Loopback redirect must match what's registered with the identity provider.
pub const REDIRECT_HOST: &str = "127.0.0.1";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OAuthProvider {
    Microsoft,
    Google,
}

impl OAuthProvider {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "microsoft" => Ok(OAuthProvider::Microsoft),
            "google" => Ok(OAuthProvider::Google),
            other => Err(format!(
                "Unknown sign-in provider '{other}' (expected 'microsoft' or 'google')"
            )),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            OAuthProvider::Microsoft => "microsoft",
            OAuthProvider::Google => "google",
        }
    }

    pub fn identity_provider(&self) -> IdentityProvider {
        match self {
            OAuthProvider::Microsoft => IdentityProvider::Microsoft,
            OAuthProvider::Google => IdentityProvider::Google,
        }
    }

    pub fn authorize_endpoint(&self) -> &'static str {
        match self {
            OAuthProvider::Microsoft => {
                "https://login.microsoftonline.com/common/oauth2/v2.0/authorize"
            }
            OAuthProvider::Google => "https://accounts.google.com/o/oauth2/v2/auth",
        }
    }

    pub fn token_endpoint(&self) -> &'static str {
        match self {
            OAuthProvider::Microsoft => {
                "https://login.microsoftonline.com/common/oauth2/v2.0/token"
            }
            OAuthProvider::Google => "https://oauth2.googleapis.com/token",
        }
    }

    pub fn userinfo_endpoint(&self) -> &'static str {
        match self {
            OAuthProvider::Microsoft => "https://graph.microsoft.com/oidc/userinfo",
            OAuthProvider::Google => "https://openidconnect.googleapis.com/v1/userinfo",
        }
    }

    /// Identity-only scopes for account sign-in. Fabric/Power BI scopes are
    /// requested separately via integration grants.
    pub fn identity_scopes(&self) -> &'static str {
        match self {
            OAuthProvider::Microsoft => "openid email profile offline_access",
            OAuthProvider::Google => "openid email profile",
        }
    }

    pub fn identity_scope_list(&self) -> Vec<&'static str> {
        match self {
            OAuthProvider::Microsoft => {
                vec!["openid", "email", "profile", "offline_access"]
            }
            OAuthProvider::Google => vec!["openid", "email", "profile"],
        }
    }

    pub fn client_id(&self, config: &crate::config::IntegrationSettings) -> Result<String, String> {
        let (configured, env_var) = match self {
            OAuthProvider::Microsoft => (&config.microsoft_client_id, "GHOST_MS_CLIENT_ID"),
            OAuthProvider::Google => (&config.google_client_id, "GHOST_GOOGLE_CLIENT_ID"),
        };
        if let Some(id) = configured {
            if !id.trim().is_empty() {
                return Ok(id.clone());
            }
        }
        if let Ok(id) = std::env::var(env_var) {
            if !id.trim().is_empty() {
                return Ok(id);
            }
        }
        if matches!(self, OAuthProvider::Google) {
            return Ok(crate::config::BUNDLED_GOOGLE_CLIENT_ID.to_string());
        }
        Err(format!(
            "Sign in with {} isn't configured yet — set integrations.{}_client_id in Settings \
             (or {env_var} for local development) to a client ID from your own app registration.",
            match self {
                OAuthProvider::Microsoft => "Microsoft",
                OAuthProvider::Google => "Google",
            },
            self.name(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_parse_round_trips_known_names() {
        assert_eq!(
            OAuthProvider::parse("microsoft").unwrap().name(),
            "microsoft"
        );
        assert_eq!(OAuthProvider::parse("google").unwrap().name(), "google");
        assert!(OAuthProvider::parse("apple").is_err());
    }

    #[test]
    fn client_id_prefers_config_over_env() {
        let _env = crate::test_support::EnvVarGuard::set("GHOST_MS_CLIENT_ID", "env-id");
        let config = crate::config::IntegrationSettings {
            microsoft_client_id: Some("configured-id".to_string()),
            google_client_id: None,
        };
        assert_eq!(
            OAuthProvider::Microsoft.client_id(&config).unwrap(),
            "configured-id"
        );
    }

    #[test]
    fn client_id_falls_back_to_env_var() {
        let _env = crate::test_support::EnvVarGuard::set("GHOST_GOOGLE_CLIENT_ID", "env-google-id");
        let config = crate::config::IntegrationSettings::default();
        assert_eq!(
            OAuthProvider::Google.client_id(&config).unwrap(),
            "env-google-id"
        );
    }

    #[test]
    fn client_id_errors_when_unconfigured() {
        let _env = crate::test_support::EnvVarGuard::remove("GHOST_MS_CLIENT_ID");
        let config = crate::config::IntegrationSettings::default();
        let err = OAuthProvider::Microsoft.client_id(&config).unwrap_err();
        assert!(err.contains("Microsoft"));
    }

    #[test]
    fn google_client_id_falls_back_to_bundled_default() {
        let _env = crate::test_support::EnvVarGuard::remove("GHOST_GOOGLE_CLIENT_ID");
        let config = crate::config::IntegrationSettings::default();
        assert_eq!(
            OAuthProvider::Google.client_id(&config).unwrap(),
            crate::config::BUNDLED_GOOGLE_CLIENT_ID
        );
    }
}
