use async_trait::async_trait;

use super::capability::{ProviderCapabilities, ProviderHealth, ProviderId};
use super::errors::ProviderError;
use super::schema::{PlanningRequest, PlanningSuggestion};

/// Internal intelligence boundary: structured suggestions only, never execution.
#[async_trait]
pub trait IntelligenceProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn capabilities(&self) -> ProviderCapabilities;
    async fn health_check(&self) -> Result<ProviderHealth, ProviderError>;
    async fn propose_plan(
        &self,
        request: PlanningRequest,
    ) -> Result<PlanningSuggestion, ProviderError>;
}
