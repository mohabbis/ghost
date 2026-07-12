use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;

use super::capability::{ProviderCapabilities, ProviderHealth, ProviderId};
use super::credentials::{CredentialProvider, CredentialStore};
use super::errors::ProviderError;
use super::parse::parse_planning_suggestion;
use super::prompt::{planning_system_prompt, planning_user_prompt};
use super::provider::IntelligenceProvider;
use super::redaction::redact_planning_request;
use super::schema::{PlanningRequest, PlanningSuggestion};
use crate::auth::AuthManager;
use crate::config::AnthropicIntelligenceConfig;
use std::path::PathBuf;

const ANTHROPIC_API: &str = "https://api.anthropic.com/v1/messages";

pub struct AnthropicProvider {
    config: AnthropicIntelligenceConfig,
    credentials: CredentialStore,
    auth_path: PathBuf,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(
        config: AnthropicIntelligenceConfig,
        credentials: CredentialStore,
        auth_path: PathBuf,
    ) -> Result<Self, ProviderError> {
        let timeout = Duration::from_secs(config.timeout_seconds.max(1));
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| ProviderError::ProviderUnavailable)?;
        Ok(AnthropicProvider {
            config,
            credentials,
            auth_path,
            client,
        })
    }

    fn auth(&self) -> AuthManager {
        AuthManager::with_path(self.auth_path.clone())
    }

    fn api_key(&self) -> Result<String, ProviderError> {
        self.credentials
            .get(&self.auth(), CredentialProvider::Anthropic)
            .ok_or(ProviderError::NotConfigured)
    }
}

#[async_trait]
impl IntelligenceProvider for AnthropicProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Anthropic
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_planning: true,
            supports_classification: true,
            supports_explanation: false,
            runs_locally: false,
            requires_network: true,
        }
    }

    async fn health_check(&self) -> Result<ProviderHealth, ProviderError> {
        if self.api_key().is_err() {
            return Ok(ProviderHealth::NotConfigured);
        }
        // Minimal non-mutating ping: empty-ish messages request would cost tokens;
        // configured + prior success is enough for status; test command does real call.
        Ok(ProviderHealth::Healthy)
    }

    async fn propose_plan(
        &self,
        request: PlanningRequest,
    ) -> Result<PlanningSuggestion, ProviderError> {
        let api_key = self.api_key()?;
        let redacted = redact_planning_request(request);
        let user_content = format!(
            "{}\n\n{}",
            planning_system_prompt(),
            planning_user_prompt(&redacted)
        );
        if user_content.len() > self.config.max_input_bytes {
            return Err(ProviderError::InvalidResponse);
        }

        let response = self
            .client
            .post(ANTHROPIC_API)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": self.config.model,
                "max_tokens": 2048,
                "messages": [
                    {"role": "user", "content": user_content},
                ],
            }))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout
                } else {
                    ProviderError::NetworkUnavailable
                }
            })?;

        if response.status().as_u16() == 429 {
            return Err(ProviderError::RateLimited);
        }
        if response.status().as_u16() == 401 {
            return Err(ProviderError::NotConfigured);
        }
        if !response.status().is_success() {
            return Err(ProviderError::ProviderUnavailable);
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|_| ProviderError::InvalidResponse)?;
        let content = body
            .pointer("/content/0/text")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::InvalidResponse)?;

        let mut suggestion = parse_planning_suggestion(content)?;
        suggestion.provider_id = Some(ProviderId::Anthropic);
        Ok(suggestion)
    }
}
