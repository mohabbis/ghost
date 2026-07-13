use super::anthropic::AnthropicProvider;
use super::capability::ProviderId;
use super::credentials::CredentialStore;
use super::local::LocalCompatibleProvider;
use super::openai::OpenAiProvider;
use super::registry::ProviderRegistry;
use super::router::{ProviderRouter, RoutingPolicy};
use crate::auth::AuthManager;
use crate::config::GhostConfig;
use std::path::PathBuf;

pub fn provider_id_from_config(s: &str) -> ProviderId {
    match s {
        "openai" => ProviderId::OpenAi,
        "anthropic" => ProviderId::Anthropic,
        "local_ollama" => ProviderId::LocalOllama,
        "local_lm_studio" => ProviderId::LocalLmStudio,
        "local" | "local_openai_compatible" => ProviderId::LocalOpenAiCompatible,
        _ => ProviderId::Disabled,
    }
}

fn local_provider_id(backend: &str) -> ProviderId {
    match backend {
        "ollama" => ProviderId::LocalOllama,
        "lm_studio" => ProviderId::LocalLmStudio,
        _ => ProviderId::LocalOpenAiCompatible,
    }
}

pub fn build_router(config: &GhostConfig, auth_path: PathBuf) -> ProviderRouter {
    let credentials = CredentialStore::new();
    let mut registry = ProviderRegistry::with_disabled_default();

    if let Ok(openai) = OpenAiProvider::new(
        config.intelligence.openai.clone(),
        credentials.clone(),
        auth_path.clone(),
    ) {
        registry.register(std::sync::Arc::new(openai));
    }

    if let Ok(anthropic) = AnthropicProvider::new(
        config.intelligence.anthropic.clone(),
        credentials,
        auth_path,
    ) {
        registry.register(std::sync::Arc::new(anthropic));
    }

    if config.intelligence.local.endpoint_is_allowed() {
        let id = local_provider_id(&config.intelligence.local.backend);
        if let Ok(local) = LocalCompatibleProvider::new(config.intelligence.local.clone(), id) {
            registry.register(std::sync::Arc::new(local));
        }
    }

    let routing = RoutingPolicy {
        default_provider: Some(provider_id_from_config(
            &config.intelligence.default_provider,
        )),
        allow_fallback: config.intelligence.routing.allow_fallback,
        local_only_for_sensitive_data: config.intelligence.routing.local_only_for_sensitive_data,
        maximum_remote_payload_bytes: config.intelligence.routing.maximum_remote_payload_bytes,
    };

    ProviderRouter::new(registry, routing)
}

pub fn auth_manager_from_path(path: PathBuf) -> AuthManager {
    AuthManager::with_path(path)
}
