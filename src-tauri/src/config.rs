//! Configuration management for Ghost application
//! Provides centralized settings with validation and persistence

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostConfig {
    /// General application settings
    pub general: GeneralSettings,
    /// Recording settings
    pub recording: RecordingSettings,
    /// Replay settings
    pub replay: ReplaySettings,
    /// AI/LLM settings
    pub ai: AISettings,
    /// Privacy settings
    pub privacy: PrivacySettings,
    /// Performance settings
    pub performance: PerformanceSettings,
    /// Audit retention settings. `#[serde(default)]` keeps config files written
    /// before this section existed loading cleanly (they get "keep everything").
    #[serde(default)]
    pub audit: AuditSettings,
    /// OAuth client IDs for account sign-in (`commands/account.rs`) and, later,
    /// stack integrations (Fabric/Power BI, Google Cloud). `#[serde(default)]`
    /// keeps configs written before this section existed loading cleanly.
    #[serde(default)]
    pub integrations: IntegrationSettings,
    /// Internal intelligence providers (suggestion-only planning). API keys are
    /// stored separately in encrypted `intelligence-secrets.json`, not here.
    #[serde(default)]
    pub intelligence: IntelligenceSettings,
}

/// Public-client OAuth identifiers. These are not secrets (PKCE public
/// clients have none to keep) — they identify Ghost to Microsoft/Google as
/// the calling application, same as a package/bundle ID. Empty means the
/// corresponding "Sign in with ..." option is unavailable until an operator
/// registers an app and fills these in (or sets `GHOST_MS_CLIENT_ID` /
/// `GHOST_GOOGLE_CLIENT_ID`, which take priority for local dev).
///
/// Google sign-in also falls back to [`BUNDLED_GOOGLE_CLIENT_ID`] when unset.
pub const BUNDLED_GOOGLE_CLIENT_ID: &str =
    "126188596343-s62mdpmhghit8q66pmujpifg9mlqqtur.apps.googleusercontent.com";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntegrationSettings {
    #[serde(default)]
    pub microsoft_client_id: Option<String>,
    #[serde(default)]
    pub google_client_id: Option<String>,
}

impl IntegrationSettings {
    /// Fill bundled OAuth client IDs when the on-disk config predates them.
    pub fn ensure_bundled_defaults(&mut self) {
        if self
            .google_client_id
            .as_ref()
            .is_none_or(|id| id.trim().is_empty())
        {
            self.google_client_id = Some(BUNDLED_GOOGLE_CLIENT_ID.to_string());
        }
    }
}

/// Settings for Ghost-owned intelligence providers (`intelligence/`).
/// Keys live in `intelligence/credentials.rs`, never in this file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceSettings {
    /// `disabled`, `openai`, or `anthropic`
    #[serde(default = "default_intelligence_provider")]
    pub default_provider: String,
    #[serde(default)]
    pub openai: OpenAiIntelligenceConfig,
    #[serde(default)]
    pub anthropic: AnthropicIntelligenceConfig,
    #[serde(default)]
    pub local: LocalIntelligenceConfig,
    #[serde(default)]
    pub routing: IntelligenceRoutingConfig,
}

fn default_intelligence_provider() -> String {
    "disabled".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiIntelligenceConfig {
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default = "default_openai_model")]
    pub model: String,
    #[serde(default = "default_provider_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_max_input_bytes")]
    pub max_input_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicIntelligenceConfig {
    #[serde(default = "default_anthropic_model")]
    pub model: String,
    #[serde(default = "default_provider_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_max_input_bytes")]
    pub max_input_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalIntelligenceConfig {
    /// `ollama`, `lm_studio`, or `openai_compatible`
    #[serde(default = "default_local_backend")]
    pub backend: String,
    #[serde(default = "default_ollama_base_url")]
    pub base_url: String,
    #[serde(default = "default_local_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_provider_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub allow_network_lan: bool,
    #[serde(default = "default_max_input_bytes")]
    pub max_input_bytes: usize,
}

impl LocalIntelligenceConfig {
    pub fn is_localhost(&self) -> bool {
        let url = self.base_url.to_lowercase();
        url.starts_with("http://127.0.0.1")
            || url.starts_with("http://localhost")
            || url.starts_with("http://[::1]")
    }

    pub fn endpoint_is_allowed(&self) -> bool {
        if self.base_url.trim().is_empty() || self.model.trim().is_empty() {
            return false;
        }
        if self.is_localhost() {
            return true;
        }
        self.allow_network_lan
    }
}

fn default_local_backend() -> String {
    "ollama".to_string()
}

fn default_ollama_base_url() -> String {
    "http://127.0.0.1:11434/v1".to_string()
}

fn default_local_model() -> String {
    "llama3.2".to_string()
}

impl Default for LocalIntelligenceConfig {
    fn default() -> Self {
        Self {
            backend: default_local_backend(),
            base_url: default_ollama_base_url(),
            model: default_local_model(),
            api_key: None,
            timeout_seconds: default_provider_timeout(),
            allow_network_lan: false,
            max_input_bytes: default_max_input_bytes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceRoutingConfig {
    #[serde(default)]
    pub allow_fallback: bool,
    #[serde(default = "default_local_only_sensitive")]
    pub local_only_for_sensitive_data: bool,
    #[serde(default = "default_max_remote_payload")]
    pub maximum_remote_payload_bytes: usize,
}

fn default_openai_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_anthropic_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

fn default_provider_timeout() -> u64 {
    60
}

fn default_max_input_bytes() -> usize {
    32_768
}

fn default_local_only_sensitive() -> bool {
    true
}

fn default_max_remote_payload() -> usize {
    32_768
}

impl Default for OpenAiIntelligenceConfig {
    fn default() -> Self {
        Self {
            api_base: None,
            model: default_openai_model(),
            timeout_seconds: default_provider_timeout(),
            max_input_bytes: default_max_input_bytes(),
        }
    }
}

impl Default for AnthropicIntelligenceConfig {
    fn default() -> Self {
        Self {
            model: default_anthropic_model(),
            timeout_seconds: default_provider_timeout(),
            max_input_bytes: default_max_input_bytes(),
        }
    }
}

impl Default for IntelligenceRoutingConfig {
    fn default() -> Self {
        Self {
            allow_fallback: false,
            local_only_for_sensitive_data: default_local_only_sensitive(),
            maximum_remote_payload_bytes: default_max_remote_payload(),
        }
    }
}

impl Default for IntelligenceSettings {
    fn default() -> Self {
        Self {
            default_provider: default_intelligence_provider(),
            openai: OpenAiIntelligenceConfig::default(),
            anthropic: AnthropicIntelligenceConfig::default(),
            local: LocalIntelligenceConfig::default(),
            routing: IntelligenceRoutingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    /// Enable auto-save of workflows
    pub auto_save: bool,
    /// Auto-save interval in seconds
    pub auto_save_interval_secs: u64,
    /// Theme preference (light, dark, auto)
    pub theme: String,
    /// Language preference
    pub language: String,
    /// Enable notifications
    pub notifications_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSettings {
    /// Capture mouse movements
    pub capture_mouse_moves: bool,
    /// Capture keyboard input
    pub capture_keyboard: bool,
    /// Minimum delay between events (ms)
    pub min_event_delay_ms: u64,
    /// Maximum recording duration (seconds, 0 = unlimited)
    pub max_duration_secs: u64,
    /// Auto-stop on idle (seconds, 0 = disabled)
    pub auto_stop_idle_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySettings {
    /// Default playback speed
    pub default_speed: f32,
    /// Enable visual verification
    pub visual_verification: bool,
    /// Visual similarity threshold (0.0-1.0)
    pub visual_threshold: f32,
    /// Max retry attempts for failed actions
    pub max_retry_attempts: u32,
    /// Base retry backoff in milliseconds
    #[serde(default = "default_retry_backoff_ms")]
    pub retry_backoff_ms: u64,
    /// Retry backoff multiplier
    pub retry_backoff_multiplier: f32,
    /// Enable self-healing (auto-adapt to UI changes)
    pub self_healing: bool,
}

/// Default base retry backoff (ms); kept as a free fn so older config files
/// that predate this field still deserialize with a sane value.
fn default_retry_backoff_ms() -> u64 {
    500
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AISettings {
    /// Enable AI features
    pub enabled: bool,
    /// LLM provider (openai, anthropic, local, local_model)
    pub provider: String,
    /// API endpoint (optional for custom providers)
    pub api_endpoint: Option<String>,
    /// Model name
    pub model: String,
    /// Enable workflow optimization suggestions
    pub auto_optimize: bool,
    /// Enable proactive suggestions
    pub proactive_suggestions: bool,
    /// Path to a local GGUF model file, used when `provider` is
    /// `local_model` (see `core::local_llm`). Nothing is loaded until this is
    /// set — no model ships with the app. `#[serde(default)]` keeps configs
    /// written before this field existed loading cleanly.
    #[serde(default)]
    pub local_model_path: Option<String>,
    /// Path to the matching `tokenizer.json`, used alongside
    /// `local_model_path`. `#[serde(default)]` for the same reason.
    #[serde(default)]
    pub local_tokenizer_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySettings {
    /// Anonymize sensitive data in logs
    pub anonymize_logs: bool,
    /// Exclude specific apps from recording
    pub excluded_apps: Vec<String>,
    /// Mask password fields
    pub mask_passwords: bool,
    /// Enable telemetry (opt-in)
    pub telemetry_enabled: bool,
    /// Local-only mode (no cloud sync)
    pub local_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    /// Enable performance profiling
    pub profiling_enabled: bool,
    /// Event buffer size
    pub event_buffer_size: usize,
    /// Thread pool size for parallel operations
    pub thread_pool_size: usize,
    /// Enable caching
    pub cache_enabled: bool,
    /// Cache size in MB
    pub cache_size_mb: usize,
    /// Capture a small screenshot crop around each recorded click, for the
    /// template-match replay fallback (`core::template_match`,
    /// `ElementInfo::template_png`). Off by default — it adds a screenshot
    /// capture's latency to every recorded click. `#[serde(default)]` so
    /// configs written before this setting existed keep loading (and stay
    /// off, matching pre-existing behavior).
    #[serde(default)]
    pub capture_element_templates: bool,
}

/// Retention policy for the Organizer's stored execution history (the audit
/// log). Deleting audit history is opt-in and user-set — both bounds default to
/// `None`, meaning Ghost keeps everything until the user chooses otherwise.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditSettings {
    /// Keep only the most recent N Organizer runs. `None` keeps all runs.
    #[serde(default)]
    pub retention_keep_last: Option<usize>,
    /// Delete Organizer runs older than N days. `None` keeps all runs.
    #[serde(default)]
    pub retention_keep_days: Option<u64>,
}

impl Default for GhostConfig {
    fn default() -> Self {
        Self {
            general: GeneralSettings {
                auto_save: true,
                auto_save_interval_secs: 300,
                theme: "auto".to_string(),
                language: "en".to_string(),
                notifications_enabled: true,
            },
            recording: RecordingSettings {
                capture_mouse_moves: false,
                capture_keyboard: true,
                min_event_delay_ms: 10,
                max_duration_secs: 3600,
                auto_stop_idle_secs: 300,
            },
            replay: ReplaySettings {
                default_speed: 1.0,
                visual_verification: false,
                visual_threshold: 0.95,
                max_retry_attempts: 3,
                retry_backoff_ms: 500,
                retry_backoff_multiplier: 1.5,
                self_healing: true,
            },
            ai: AISettings {
                enabled: false,
                provider: "local".to_string(),
                api_endpoint: None,
                model: "local-heuristics".to_string(),
                auto_optimize: false,
                proactive_suggestions: false,
                local_model_path: None,
                local_tokenizer_path: None,
            },
            privacy: PrivacySettings {
                anonymize_logs: true,
                excluded_apps: vec![],
                mask_passwords: true,
                telemetry_enabled: false,
                local_only: true,
            },
            performance: PerformanceSettings {
                profiling_enabled: false,
                event_buffer_size: 10000,
                thread_pool_size: 4,
                cache_enabled: true,
                cache_size_mb: 100,
                capture_element_templates: false,
            },
            // Keep all audit history by default; retention is opt-in.
            audit: AuditSettings::default(),
            integrations: IntegrationSettings {
                microsoft_client_id: None,
                google_client_id: Some(BUNDLED_GOOGLE_CLIENT_ID.to_string()),
            },
            intelligence: IntelligenceSettings::default(),
        }
    }
}

impl GhostConfig {
    /// Load configuration from disk or create default
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let mut config: GhostConfig = serde_json::from_str(&content)?;
            config.integrations.ensure_bundled_defaults();
            Ok(config)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to disk
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;

        Ok(())
    }

    /// Get the configuration file path
    fn config_path() -> Result<PathBuf> {
        let data_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

        Ok(data_dir.join("ghost").join("config.json"))
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        if self.replay.default_speed <= 0.0 || self.replay.default_speed > 10.0 {
            anyhow::bail!("Invalid playback speed: must be between 0.0 and 10.0");
        }

        if self.replay.visual_threshold < 0.0 || self.replay.visual_threshold > 1.0 {
            anyhow::bail!("Invalid visual threshold: must be between 0.0 and 1.0");
        }

        if self.performance.thread_pool_size == 0 {
            anyhow::bail!("Thread pool size must be at least 1");
        }

        // A retention bound of 0 would delete all history the moment a run is
        // saved. Disabling retention is expressed as `None`, not `Some(0)`.
        if self.audit.retention_keep_last == Some(0) {
            anyhow::bail!("Audit retention keep-last must be at least 1 (use null to keep all)");
        }
        if self.audit.retention_keep_days == Some(0) {
            anyhow::bail!("Audit retention keep-days must be at least 1 (use null to keep all)");
        }

        match self.intelligence.default_provider.as_str() {
            "disabled"
            | "openai"
            | "anthropic"
            | "local_ollama"
            | "local_lm_studio"
            | "local"
            | "local_openai_compatible" => {}
            other => anyhow::bail!(
                "Invalid intelligence default provider '{other}' (expected disabled, openai, anthropic, or local_*)"
            ),
        }

        if self.intelligence.openai.timeout_seconds == 0 {
            anyhow::bail!("OpenAI intelligence timeout must be at least 1 second");
        }
        if self.intelligence.anthropic.timeout_seconds == 0 {
            anyhow::bail!("Anthropic intelligence timeout must be at least 1 second");
        }
        if self.intelligence.local.timeout_seconds == 0 {
            anyhow::bail!("Local intelligence timeout must be at least 1 second");
        }
        if !self.intelligence.local.base_url.trim().is_empty()
            && !self.intelligence.local.endpoint_is_allowed()
        {
            anyhow::bail!(
                "Local model endpoint must be localhost unless allow_network_lan is enabled"
            );
        }

        Ok(())
    }

    /// Reset to default configuration
    pub fn reset() -> Result<Self> {
        let config = Self::default();
        config.save()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GhostConfig::default();
        assert!(config.validate().is_ok());
    }

    /// Hard non-goal: no silent/automatic template capture into recordings or
    /// Action Plans. Opt-in only via `capture_element_templates`.
    #[test]
    fn capture_element_templates_defaults_off() {
        let config = GhostConfig::default();
        assert!(
            !config.performance.capture_element_templates,
            "template capture must stay opt-in (off by default)"
        );
        // Missing field in older configs must also deserialize as off.
        let json = r#"{"profiling_enabled":false,"event_buffer_size":1,"thread_pool_size":1,"cache_enabled":false,"cache_size_mb":1}"#;
        let perf: PerformanceSettings = serde_json::from_str(json).unwrap();
        assert!(!perf.capture_element_templates);
    }

    #[test]
    fn test_invalid_speed() {
        let mut config = GhostConfig::default();
        config.replay.default_speed = -1.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_serialization() {
        let config = GhostConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: GhostConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.general.theme, deserialized.general.theme);
    }

    /// Default keeps everything: both retention bounds are unset.
    #[test]
    fn audit_retention_defaults_to_keep_all() {
        let config = GhostConfig::default();
        assert_eq!(config.audit.retention_keep_last, None);
        assert_eq!(config.audit.retention_keep_days, None);
    }

    /// A zero retention bound is rejected — disabling retention uses null.
    #[test]
    fn zero_retention_bound_is_rejected() {
        let mut config = GhostConfig::default();
        config.audit.retention_keep_last = Some(0);
        assert!(config.validate().is_err());

        let mut config = GhostConfig::default();
        config.audit.retention_keep_days = Some(0);
        assert!(config.validate().is_err());

        // A positive bound is fine.
        let mut config = GhostConfig::default();
        config.audit.retention_keep_last = Some(50);
        assert!(config.validate().is_ok());
    }

    /// A config file written before the `audit` section existed must still load
    /// (the field is `#[serde(default)]`), yielding keep-everything.
    #[test]
    fn config_without_audit_section_deserializes_to_keep_all() {
        let mut value = serde_json::to_value(GhostConfig::default()).unwrap();
        value.as_object_mut().unwrap().remove("audit");
        let config: GhostConfig = serde_json::from_value(value).unwrap();
        assert_eq!(config.audit.retention_keep_last, None);
        assert_eq!(config.audit.retention_keep_days, None);
    }

    #[test]
    fn ensure_bundled_defaults_fills_missing_google_client_id() {
        let mut integrations = IntegrationSettings::default();
        integrations.ensure_bundled_defaults();
        assert_eq!(
            integrations.google_client_id.as_deref(),
            Some(BUNDLED_GOOGLE_CLIENT_ID)
        );
    }
}

// Made with Bob
