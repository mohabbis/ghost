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
    /// Retry backoff multiplier
    pub retry_backoff_multiplier: f32,
    /// Enable self-healing (auto-adapt to UI changes)
    pub self_healing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AISettings {
    /// Enable AI features
    pub enabled: bool,
    /// LLM provider (openai, anthropic, local)
    pub provider: String,
    /// API endpoint (optional for custom providers)
    pub api_endpoint: Option<String>,
    /// Model name
    pub model: String,
    /// Enable workflow optimization suggestions
    pub auto_optimize: bool,
    /// Enable proactive suggestions
    pub proactive_suggestions: bool,
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
                retry_backoff_multiplier: 1.5,
                self_healing: true,
            },
            ai: AISettings {
                enabled: true,
                provider: "local".to_string(),
                api_endpoint: None,
                model: "gpt-4".to_string(),
                auto_optimize: true,
                proactive_suggestions: true,
            },
            privacy: PrivacySettings {
                anonymize_logs: true,
                excluded_apps: vec![],
                mask_passwords: true,
                telemetry_enabled: false,
                local_only: false,
            },
            performance: PerformanceSettings {
                profiling_enabled: false,
                event_buffer_size: 10000,
                thread_pool_size: 4,
                cache_enabled: true,
                cache_size_mb: 100,
            },
        }
    }
}

impl GhostConfig {
    /// Load configuration from disk or create default
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: GhostConfig = serde_json::from_str(&content)?;
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
}

// Made with Bob
