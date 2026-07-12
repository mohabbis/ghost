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
use crate::config::OpenAiIntelligenceConfig;
use std::path::PathBuf;

pub struct OpenAiProvider {
    config: OpenAiIntelligenceConfig,
    credentials: CredentialStore,
    auth_path: PathBuf,
    client: Client,
}

impl OpenAiProvider {
    pub fn new(
        config: OpenAiIntelligenceConfig,
        credentials: CredentialStore,
        auth_path: PathBuf,
    ) -> Result<Self, ProviderError> {
        let timeout = Duration::from_secs(config.timeout_seconds.max(1));
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| ProviderError::ProviderUnavailable)?;
        Ok(OpenAiProvider {
            config,
            credentials,
            auth_path,
            client,
        })
    }

    fn auth(&self) -> AuthManager {
        AuthManager::with_path(self.auth_path.clone())
    }

    fn api_base(&self) -> &str {
        self.config
            .api_base
            .as_deref()
            .unwrap_or("https://api.openai.com/v1")
    }

    fn api_key(&self) -> Result<String, ProviderError> {
        self.credentials
            .get(&self.auth(), CredentialProvider::OpenAi)
            .ok_or(ProviderError::NotConfigured)
    }
}

#[async_trait]
impl IntelligenceProvider for OpenAiProvider {
    fn id(&self) -> ProviderId {
        ProviderId::OpenAi
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
        let api_key = self.api_key()?;
        let url = format!("{}/models", self.api_base().trim_end_matches('/'));
        match self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => Ok(ProviderHealth::Healthy),
            Ok(resp) if resp.status().as_u16() == 401 => Ok(ProviderHealth::Unavailable {
                reason: "Invalid OpenAI API key".to_string(),
            }),
            Ok(resp) => Ok(ProviderHealth::Degraded {
                reason: format!("OpenAI returned HTTP {}", resp.status()),
            }),
            Err(e) if e.is_timeout() => Ok(ProviderHealth::Unavailable {
                reason: "OpenAI request timed out".to_string(),
            }),
            Err(_) => Ok(ProviderHealth::Unavailable {
                reason: "Could not reach OpenAI".to_string(),
            }),
        }
    }

    async fn propose_plan(
        &self,
        request: PlanningRequest,
    ) -> Result<PlanningSuggestion, ProviderError> {
        let api_key = self.api_key()?;
        let redacted = redact_planning_request(request);
        let payload_len = planning_user_prompt(&redacted).len();
        if payload_len > self.config.max_input_bytes {
            return Err(ProviderError::InvalidResponse);
        }

        let url = format!("{}/chat/completions", self.api_base().trim_end_matches('/'));
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": self.config.model,
                "messages": [
                    {"role": "system", "content": planning_system_prompt()},
                    {"role": "user", "content": planning_user_prompt(&redacted)},
                ],
                "temperature": 0.2,
                "response_format": {"type": "json_object"},
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
        if !response.status().is_success() {
            return Err(ProviderError::ProviderUnavailable);
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|_| ProviderError::InvalidResponse)?;
        let content = body
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::InvalidResponse)?;

        let mut suggestion = parse_planning_suggestion(content)?;
        suggestion.provider_id = Some(ProviderId::OpenAi);
        Ok(suggestion)
    }
}
