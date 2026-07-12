use super::capability::ProviderId;
use super::errors::ProviderError;
use super::registry::ProviderRegistry;
use super::schema::{PlanningRequest, PlanningSuggestion};

#[derive(Clone, Debug, Default)]
pub struct RoutingPolicy {
    pub default_provider: Option<ProviderId>,
    pub allow_fallback: bool,
    pub local_only_for_sensitive_data: bool,
    pub maximum_remote_payload_bytes: usize,
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

    pub async fn propose_plan(
        &self,
        request: PlanningRequest,
    ) -> Result<PlanningSuggestion, ProviderError> {
        let provider_id = self.policy.default_provider.unwrap_or(ProviderId::Disabled);
        let provider = self
            .registry
            .get(provider_id)
            .ok_or(ProviderError::NotConfigured)?;
        provider.propose_plan(request).await
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
        assert!(matches!(result, Err(ProviderError::NotConfigured)));
    }
}
