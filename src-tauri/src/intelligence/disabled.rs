use async_trait::async_trait;

use super::capability::{ProviderCapabilities, ProviderHealth, ProviderId};
use super::errors::ProviderError;
use super::provider::IntelligenceProvider;
use super::schema::{PlanningRequest, PlanningSuggestion};

/// Default when no intelligence provider is configured.
#[derive(Default)]
pub struct DisabledProvider;

#[async_trait]
impl IntelligenceProvider for DisabledProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Disabled
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    async fn health_check(&self) -> Result<ProviderHealth, ProviderError> {
        Ok(ProviderHealth::NotConfigured)
    }

    async fn propose_plan(
        &self,
        _request: PlanningRequest,
    ) -> Result<PlanningSuggestion, ProviderError> {
        Err(ProviderError::NotConfigured)
    }
}
