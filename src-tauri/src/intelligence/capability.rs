use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Disabled,
    OpenAi,
    Anthropic,
    LocalOllama,
    LocalLmStudio,
    LocalOpenAiCompatible,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Disabled => "disabled",
            ProviderId::OpenAi => "openai",
            ProviderId::Anthropic => "anthropic",
            ProviderId::LocalOllama => "local_ollama",
            ProviderId::LocalLmStudio => "local_lm_studio",
            ProviderId::LocalOpenAiCompatible => "local_openai_compatible",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_planning: bool,
    pub supports_classification: bool,
    pub supports_explanation: bool,
    pub runs_locally: bool,
    pub requires_network: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderHealth {
    Healthy,
    Degraded { reason: String },
    Unavailable { reason: String },
    NotConfigured,
}
