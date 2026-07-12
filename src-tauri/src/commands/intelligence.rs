//! Internal intelligence provider commands (suggestion-only, no execution).
//!
//! Command: `intelligence_provider_status` | risk: `safe-read`
//! Command: `intelligence_set_api_key` | risk: `local-mutate` | stores encrypted key
//! Command: `intelligence_clear_api_key` | risk: `local-mutate`
//! Command: `intelligence_test_provider` | risk: `external-mutate` | network health check
//! Command: `intelligence_propose_plan` | risk: `external-mutate` | sends redacted metadata remotely; returns suggestion only

use crate::engine::GhostEngine;
use crate::intelligence::capability::{ProviderHealth, ProviderId};
use crate::intelligence::credentials::{CredentialProvider, CredentialStore};
use crate::intelligence::IntelligenceProvider;
use crate::intelligence::schema::PlanningRequest;
use crate::intelligence::service::{build_router, provider_id_from_config};
use tauri::State;

#[derive(serde::Serialize)]
pub struct IntelligenceProviderStatusEntry {
    pub id: String,
    pub configured: bool,
    pub model: String,
    pub runs_locally: bool,
    pub requires_network: bool,
    pub health: ProviderHealth,
}

#[derive(serde::Serialize)]
pub struct IntelligenceProviderStatus {
    pub default_provider: String,
    pub providers: Vec<IntelligenceProviderStatusEntry>,
}

#[tauri::command]
pub async fn intelligence_provider_status(
    engine: State<'_, GhostEngine>,
) -> Result<IntelligenceProviderStatus, String> {
    let config = engine.get_config();
    let auth = engine.auth();
    let credentials = CredentialStore::new();
    let router = build_router(&config, auth.storage_path().clone());

    let mut providers = Vec::new();

    for (id, cred, model) in [
        (
            ProviderId::OpenAi,
            CredentialProvider::OpenAi,
            config.intelligence.openai.model.clone(),
        ),
        (
            ProviderId::Anthropic,
            CredentialProvider::Anthropic,
            config.intelligence.anthropic.model.clone(),
        ),
    ] {
        let configured = credentials.is_configured(&auth, cred);
        let entry_provider = router
            .registry()
            .get(id)
            .ok_or_else(|| format!("Provider {} not registered", id.as_str()))?;
        let caps = IntelligenceProvider::capabilities(entry_provider.as_ref());
        let health = if configured {
            IntelligenceProvider::health_check(entry_provider.as_ref())
                .await
                .map_err(|e| e.to_string())?
        } else {
            ProviderHealth::NotConfigured
        };
        providers.push(IntelligenceProviderStatusEntry {
            id: id.as_str().to_string(),
            configured,
            model,
            runs_locally: caps.runs_locally,
            requires_network: caps.requires_network,
            health,
        });
    }

    Ok(IntelligenceProviderStatus {
        default_provider: config.intelligence.default_provider.clone(),
        providers,
    })
}

#[tauri::command]
pub fn intelligence_set_api_key(
    provider: String,
    api_key: String,
    engine: State<'_, GhostEngine>,
) -> Result<(), String> {
    let cred = CredentialProvider::parse(&provider)?;
    CredentialStore::new()
        .set(&engine.auth(), cred, &api_key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn intelligence_clear_api_key(
    provider: String,
    engine: State<'_, GhostEngine>,
) -> Result<(), String> {
    let cred = CredentialProvider::parse(&provider)?;
    CredentialStore::new()
        .clear(&engine.auth(), cred)
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct IntelligenceTestResult {
    pub provider: String,
    pub health: ProviderHealth,
}

#[tauri::command]
pub async fn intelligence_test_provider(
    provider: String,
    engine: State<'_, GhostEngine>,
) -> Result<IntelligenceTestResult, String> {
    let config = engine.get_config();
    let auth = engine.auth();
    let router = build_router(&config, auth.storage_path().clone());
    let id = provider_id_from_config(&provider);
    if id == ProviderId::Disabled {
        return Err("Unknown provider".to_string());
    }
    let entry = router
        .registry()
        .get(id)
        .ok_or_else(|| "Provider not available".to_string())?;
    let health = IntelligenceProvider::health_check(entry.as_ref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(IntelligenceTestResult { provider, health })
}

#[tauri::command]
pub async fn intelligence_propose_plan(
    request: PlanningRequest,
    engine: State<'_, GhostEngine>,
) -> Result<crate::intelligence::schema::PlanningSuggestion, String> {
    let config = engine.get_config();
    let auth = engine.auth();
    let router = build_router(&config, auth.storage_path().clone());
    router
        .propose_plan(request)
        .await
        .map_err(|e| e.to_string())
}
