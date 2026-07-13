use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;

use super::super::capability::{ProviderCapabilities, ProviderHealth, ProviderId};
use super::super::errors::ProviderError;
use super::super::parse::parse_planning_suggestion;
use super::super::prompt::{planning_system_prompt, planning_user_prompt};
use super::super::provider::IntelligenceProvider;
use super::super::redaction::redact_planning_request;
use super::super::schema::{PlanningRequest, PlanningSuggestion};
use crate::config::LocalIntelligenceConfig;

pub struct LocalCompatibleProvider {
    config: LocalIntelligenceConfig,
    provider_id: ProviderId,
    client: Client,
}

impl LocalCompatibleProvider {
    pub fn new(
        config: LocalIntelligenceConfig,
        provider_id: ProviderId,
    ) -> Result<Self, ProviderError> {
        if !config.endpoint_is_allowed() {
            return Err(ProviderError::NotConfigured);
        }
        let timeout = Duration::from_secs(config.timeout_seconds.max(1));
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| ProviderError::ProviderUnavailable)?;
        Ok(LocalCompatibleProvider {
            config,
            provider_id,
            client,
        })
    }

    fn chat_url(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        )
    }

    fn models_url(&self) -> String {
        format!("{}/models", self.config.base_url.trim_end_matches('/'))
    }
}

#[async_trait]
impl IntelligenceProvider for LocalCompatibleProvider {
    fn id(&self) -> ProviderId {
        self.provider_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_planning: true,
            supports_classification: true,
            supports_explanation: false,
            runs_locally: self.config.is_localhost(),
            requires_network: !self.config.is_localhost(),
        }
    }

    async fn health_check(&self) -> Result<ProviderHealth, ProviderError> {
        if self.config.model.trim().is_empty() {
            return Ok(ProviderHealth::NotConfigured);
        }
        match self.client.get(self.models_url()).send().await {
            Ok(resp) if resp.status().is_success() => Ok(ProviderHealth::Healthy),
            Ok(resp) => Ok(ProviderHealth::Degraded {
                reason: format!("Local endpoint returned HTTP {}", resp.status()),
            }),
            Err(e) if e.is_timeout() => Ok(ProviderHealth::Unavailable {
                reason: "Local model request timed out".to_string(),
            }),
            Err(_) => Ok(ProviderHealth::Unavailable {
                reason: "Could not reach local model endpoint".to_string(),
            }),
        }
    }

    async fn propose_plan(
        &self,
        request: PlanningRequest,
    ) -> Result<PlanningSuggestion, ProviderError> {
        let redacted = redact_planning_request(request);
        if planning_user_prompt(&redacted).len() > self.config.max_input_bytes {
            return Err(ProviderError::InvalidResponse);
        }

        let mut req = self
            .client
            .post(self.chat_url())
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": self.config.model,
                "messages": [
                    {"role": "system", "content": planning_system_prompt()},
                    {"role": "user", "content": planning_user_prompt(&redacted)},
                ],
                "temperature": 0.2,
                "stream": false,
            }));

        if let Some(key) = self.config.api_key.as_ref().filter(|k| !k.is_empty()) {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let response = req.send().await.map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout
            } else {
                ProviderError::NetworkUnavailable
            }
        })?;

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
        suggestion.provider_id = Some(self.provider_id);
        Ok(suggestion)
    }
}
