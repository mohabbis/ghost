//! Internal intelligence provider commands (suggestion-only, no execution).
//!
//! Stability: experimental — gated behind `--features experimental`; this
//! module is not compiled and these commands are not registered or
//! reachable in stock builds. See `docs/integrations-roadmap.md` and
//! `docs/ai-provider-architecture.md`.
//!
//! Command: `intelligence_provider_status` | risk: `safe-read`
//! Command: `intelligence_set_api_key` | risk: `local-mutate` | stores encrypted key
//! Command: `intelligence_clear_api_key` | risk: `local-mutate`
//! Command: `intelligence_test_provider` | risk: `external-mutate` | network health check
//! Command: `intelligence_propose_plan` | risk: `external-mutate` | sends redacted metadata remotely; returns suggestion only

use crate::engine::GhostEngine;
use crate::intelligence::capability::{ProviderHealth, ProviderId};
use crate::intelligence::credentials::{CredentialProvider, CredentialStore};
use crate::intelligence::discover_local_runtimes;
use crate::intelligence::schema::PlanningRequest;
use crate::intelligence::service::{build_router, provider_id_from_config};
use crate::intelligence::IntelligenceProvider;
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

    let remote = [
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
    ];
    for (id, cred, model) in remote {
        push_provider_status(
            &mut providers,
            &router,
            id,
            credentials.is_configured(&auth, cred),
            model,
        )
        .await?;
    }

    if config.intelligence.local.endpoint_is_allowed() {
        let local_id = match config.intelligence.local.backend.as_str() {
            "ollama" => ProviderId::LocalOllama,
            "lm_studio" => ProviderId::LocalLmStudio,
            _ => ProviderId::LocalOpenAiCompatible,
        };
        push_provider_status(
            &mut providers,
            &router,
            local_id,
            true,
            config.intelligence.local.model.clone(),
        )
        .await?;
    }

    Ok(IntelligenceProviderStatus {
        default_provider: config.intelligence.default_provider.clone(),
        providers,
    })
}

async fn push_provider_status(
    providers: &mut Vec<IntelligenceProviderStatusEntry>,
    router: &crate::intelligence::ProviderRouter,
    id: ProviderId,
    configured: bool,
    model: String,
) -> Result<(), String> {
    let Some(entry_provider) = router.registry().get(id) else {
        return Ok(());
    };
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
    Ok(())
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

#[tauri::command]
pub fn intelligence_discover_local() -> Vec<crate::intelligence::LocalRuntimeDiscovery> {
    discover_local_runtimes()
}

#[derive(serde::Serialize)]
pub struct OrganizerIntelligenceSuggestResult {
    pub suggestion: crate::intelligence::schema::PlanningSuggestion,
    pub files_considered: usize,
}

/// Scan a Zone's file metadata and ask the configured intelligence provider for
/// suggestion-only planning hints. Never executes filesystem mutations.
///
/// Command: `organizer_intelligence_suggest` | risk: `external-mutate` when a
/// remote provider is selected; `safe-read` when a local provider is used |
/// returns `PlanningSuggestion` only; validated via `suggestion_is_safe`.
#[tauri::command]
pub async fn organizer_intelligence_suggest(
    zone_id: String,
    engine: State<'_, GhostEngine>,
) -> Result<OrganizerIntelligenceSuggestResult, String> {
    use crate::intelligence::schema::{suggestion_is_safe, DataSensitivity, FileMetadataSummary};
    use crate::organizer::planner::scan_zone_files;
    use crate::storage::open_default;

    let config = engine.get_config();
    if config.intelligence.default_provider == "disabled" {
        return Err(
            "Intelligence provider is disabled — choose a provider in Settings".to_string(),
        );
    }

    let db = open_default().map_err(|e| e.to_string())?;
    let scanned = scan_zone_files(&db, &zone_id).map_err(|e| e.to_string())?;
    let files_considered = scanned.len();

    let file_metadata: Vec<FileMetadataSummary> = scanned
        .into_iter()
        .map(|f| {
            let zone_relative_path = f.file_name.clone();
            FileMetadataSummary {
                name: f.file_name,
                extension: f.extension,
                size_bytes: f.size,
                created_at: f
                    .created
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs().to_string()),
                zone_relative_path,
            }
        })
        .collect();

    let request = PlanningRequest {
        objective: format!("Suggest categories and folder rules for Zone {zone_id}"),
        zone_id: Some(zone_id),
        file_metadata,
        sensitivity: DataSensitivity::InternalMetadata,
    };

    let auth = engine.auth();
    let router = build_router(&config, auth.storage_path().clone());
    let suggestion = router
        .propose_plan(request)
        .await
        .map_err(|e| e.to_string())?;
    if !suggestion_is_safe(&suggestion) {
        return Err("Provider returned unsafe suggestion content — rejected".to_string());
    }

    Ok(OrganizerIntelligenceSuggestResult {
        suggestion,
        files_considered,
    })
}
