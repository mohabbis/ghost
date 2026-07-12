use std::collections::HashMap;
use std::sync::Arc;

use super::capability::{ProviderCapabilities, ProviderHealth, ProviderId};
use super::disabled::DisabledProvider;
use super::errors::ProviderError;
use super::provider::IntelligenceProvider;

pub struct ProviderRegistry {
    providers: HashMap<ProviderId, Arc<dyn IntelligenceProvider>>,
}

impl ProviderRegistry {
    pub fn with_disabled_default() -> Self {
        let mut providers: HashMap<ProviderId, Arc<dyn IntelligenceProvider>> = HashMap::new();
        providers.insert(ProviderId::Disabled, Arc::new(DisabledProvider));
        ProviderRegistry { providers }
    }

    pub fn register(&mut self, provider: Arc<dyn IntelligenceProvider>) {
        self.providers.insert(provider.id(), provider);
    }

    pub fn get(&self, id: ProviderId) -> Option<Arc<dyn IntelligenceProvider>> {
        self.providers.get(&id).cloned()
    }

    pub fn health_summary(&self) -> HashMap<ProviderId, ProviderHealth> {
        self.providers
            .keys()
            .map(|id| (*id, ProviderHealth::NotConfigured))
            .collect()
    }

    pub fn capabilities(&self, id: ProviderId) -> Result<ProviderCapabilities, ProviderError> {
        self.get(id)
            .map(|p| p.capabilities())
            .ok_or(ProviderError::NotConfigured)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::with_disabled_default()
    }
}
