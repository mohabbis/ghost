use super::capability::ProviderId;
use super::errors::ProviderError;
use super::provider::IntelligenceProvider;
use super::registry::ProviderRegistry;
use super::schema::{DataSensitivity, PlanningRequest, PlanningSuggestion};

#[derive(Clone, Debug)]
pub struct RoutingPolicy {
    pub default_provider: Option<ProviderId>,
    pub allow_fallback: bool,
    pub local_only_for_sensitive_data: bool,
    pub maximum_remote_payload_bytes: usize,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self {
            default_provider: None,
            allow_fallback: false,
            local_only_for_sensitive_data: true,
            maximum_remote_payload_bytes: 32_768,
        }
    }
}

pub struct ProviderRouter {
    registry: ProviderRegistry,
    policy: RoutingPolicy,
}

impl ProviderRouter {
    pub fn new(registry: ProviderRegistry, policy: RoutingPolicy) -> Self {
        ProviderRouter { registry, policy }
    }

    pub fn policy(&self) -> &RoutingPolicy {
        &self.policy
    }

    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    pub async fn propose_plan(
        &self,
        request: PlanningRequest,
    ) -> Result<PlanningSuggestion, ProviderError> {
        let provider_id = self.policy.default_provider.unwrap_or(ProviderId::Disabled);

        if self.policy.local_only_for_sensitive_data
            && matches!(
                request.sensitivity,
                DataSensitivity::Confidential | DataSensitivity::Secret
            )
            && provider_id != ProviderId::Disabled
        {
            return Err(ProviderError::NotConfigured);
        }

        let payload_len = serde_json::to_string(&request.file_metadata)
            .map(|s| s.len())
            .unwrap_or(0);
        if payload_len > self.policy.maximum_remote_payload_bytes {
            return Err(ProviderError::InvalidResponse);
        }

        let provider = self
            .registry
            .get(provider_id)
            .ok_or(ProviderError::NotConfigured)?;
        IntelligenceProvider::propose_plan(provider.as_ref(), request).await
    }

    pub fn set_default_provider(&mut self, id: Option<ProviderId>) {
        self.policy.default_provider = id;
    }
}

impl Default for ProviderRouter {
    fn default() -> Self {
        Self::new(ProviderRegistry::default(), RoutingPolicy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_router_uses_disabled_provider() {
        let router = ProviderRouter::default();
        let result = router
            .propose_plan(PlanningRequest {
                objective: "test".to_string(),
                zone_id: None,
                file_metadata: vec![],
                sensitivity: super::super::schema::DataSensitivity::Public,
            })
            .await;
        assert!(
            matches!(result, Err(ProviderError::NotConfigured)),
            "expected NotConfigured, got {result:?}"
        );
    }

    #[tokio::test]
    async fn confidential_data_blocked_for_remote_providers() {
        let policy = RoutingPolicy {
            default_provider: Some(ProviderId::OpenAi),
            ..Default::default()
        };
        let router = ProviderRouter::new(ProviderRegistry::default(), policy);
        let result = router
            .propose_plan(PlanningRequest {
                objective: "test".to_string(),
                zone_id: None,
                file_metadata: vec![],
                sensitivity: DataSensitivity::Secret,
            })
            .await;
        assert!(matches!(result, Err(ProviderError::NotConfigured)));
    }
}
