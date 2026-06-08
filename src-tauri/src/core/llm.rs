//! LLM integration for AI-assisted workflow generation.
//! Provides abstraction over OpenAI, Claude, and local fallback modes.

use crate::config::AISettings;
use crate::core::events::{ElementInfo, InputEvent};

use std::env;
use std::sync::{Arc, RwLock};

/// LLM provider trait for workflow generation
#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    async fn generate_workflow(
        &self,
        prompt: &str,
        screenshot: Option<&[u8]>,
        ax_tree: Option<&str>,
        element_context: &[ElementInfo],
    ) -> anyhow::Result<Vec<InputEvent>>;

    fn name(&self) -> &'static str;
}

/// LLM Configuration
#[derive(Clone, Debug)]
pub struct LLMConfig {
    pub provider: LLMProviderType,
    pub api_key: Option<String>,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    /// Optional custom base endpoint (OpenAI-compatible). `None` = provider default.
    pub endpoint: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub enum LLMProviderType {
    #[default]
    Local,
    OpenAI,
    Claude,
}

impl LLMConfig {
    pub fn from_env() -> Self {
        let provider = if let Ok(key) = env::var("OPENAI_API_KEY") {
            if !key.is_empty() {
                LLMProviderType::OpenAI
            } else {
                LLMProviderType::Local
            }
        } else if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
            if !key.is_empty() {
                LLMProviderType::Claude
            } else {
                LLMProviderType::Local
            }
        } else {
            LLMProviderType::Local
        };

        let model = match provider {
            LLMProviderType::OpenAI => {
                env::var("GHOST_AI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string())
            }
            LLMProviderType::Claude => {
                env::var("GHOST_AI_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_string())
            }
            LLMProviderType::Local => "local-heuristic".to_string(),
        };

        LLMConfig {
            provider,
            api_key: None, // Will be loaded on demand
            model,
            max_tokens: 2048,
            temperature: 0.7,
            endpoint: None,
        }
    }

    /// Build an LLM config from the persisted `AISettings`.
    ///
    /// The provider is chosen by `ai.provider` rather than by which key env var
    /// happens to be set. API keys still come from the environment only (never
    /// from `config.json`); if the selected remote provider has no key present,
    /// we fall back to `Local` so generation degrades gracefully instead of
    /// erroring at call time.
    pub fn from_ghost_config(ai: &AISettings) -> Self {
        let requested = match ai.provider.to_lowercase().as_str() {
            "openai" => LLMProviderType::OpenAI,
            "anthropic" | "claude" => LLMProviderType::Claude,
            _ => LLMProviderType::Local,
        };

        // Downgrade to Local if the matching API key is absent.
        let provider = match requested {
            LLMProviderType::OpenAI if env_key_present("OPENAI_API_KEY") => LLMProviderType::OpenAI,
            LLMProviderType::Claude if env_key_present("ANTHROPIC_API_KEY") => {
                LLMProviderType::Claude
            }
            LLMProviderType::Local => LLMProviderType::Local,
            _ => LLMProviderType::Local,
        };

        let model = match provider {
            LLMProviderType::Local => "local-heuristic".to_string(),
            _ => ai.model.clone(),
        };

        LLMConfig {
            provider,
            api_key: None,
            model,
            max_tokens: 2048,
            temperature: 0.7,
            endpoint: ai.api_endpoint.clone(),
        }
    }

    pub fn api_key(&self) -> Option<String> {
        match self.provider {
            LLMProviderType::OpenAI => env::var("OPENAI_API_KEY").ok(),
            LLMProviderType::Claude => env::var("ANTHROPIC_API_KEY").ok(),
            LLMProviderType::Local => None,
        }
    }
}

/// True if the named env var is set and non-empty.
fn env_key_present(name: &str) -> bool {
    env::var(name).map(|v| !v.is_empty()).unwrap_or(false)
}

/// Global LLM instance. A `RwLock` (not `OnceLock`) so the active provider can
/// be rebuilt when the user changes the AI settings at runtime.
static LLM_INSTANCE: RwLock<Option<Arc<dyn LLMProvider>>> = RwLock::new(None);

/// Initialize (or replace) the active LLM provider from `config`.
pub fn init_llm(config: &LLMConfig) {
    let provider: Arc<dyn LLMProvider> = match config.provider {
        LLMProviderType::OpenAI => Arc::new(OpenAIProvider::new(config)),
        LLMProviderType::Claude => Arc::new(ClaudeProvider::new(config)),
        LLMProviderType::Local => Arc::new(LocalFallback::new()),
    };
    if let Ok(mut guard) = LLM_INSTANCE.write() {
        *guard = Some(provider);
    }
}

/// Get a handle to the active LLM provider, if one has been initialized.
pub fn get_llm() -> Option<Arc<dyn LLMProvider>> {
    LLM_INSTANCE.read().ok().and_then(|guard| guard.clone())
}

/// OpenAI provider implementation
pub struct OpenAIProvider {
    config: LLMConfig,
}

impl OpenAIProvider {
    pub fn new(config: &LLMConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }
}

#[async_trait::async_trait]
impl LLMProvider for OpenAIProvider {
    async fn generate_workflow(
        &self,
        prompt: &str,
        _screenshot: Option<&[u8]>,
        ax_tree: Option<&str>,
        element_context: &[ElementInfo],
    ) -> anyhow::Result<Vec<InputEvent>> {
        let api_key = self
            .config
            .api_key()
            .ok_or_else(|| anyhow::anyhow!("OPENAI_API_KEY not set"))?;

        let client = reqwest::Client::new();

        // Build the conversation with element context
        let system_prompt = format!(
            "You are an AI automation assistant. Convert natural language commands into structured input events.\
            \n\nTask: {}\
            \n\nAvailable UI elements: {:?}\
            \n\nAccessibility tree context: {:?}\
            \n\nOutput ONLY valid JSON matching this schema: {{\"events\": [INPUT_EVENTS]}}\
            \nWhere each INPUT_EVENT is one of:\
            \n- {{\"MouseClick\": {{\"x\": int, \"y\": int, \"button\": int, \"element\": {{\"role\": str, \"name\": str, \"app\": str}}}}}}\
            \n- {{\"Key\": {{\"code\": int, \"chars\": str, \"modifiers\": int, \"action\": \"Down\"| \"Up\"}}}}\
            \n- {{\"Delay\": {{\"ms\": int}}}}\
            \n- {{ \"Scroll\": {{\"dx\": int, \"dy\": int}}}}\
            \nSet optional fields to null. Keep events array minimal but complete.",
            prompt,
            element_context.iter().take(10).collect::<Vec<_>>(),
            ax_tree.unwrap_or("no context")
        );

        let endpoint = self
            .config
            .endpoint
            .as_deref()
            .unwrap_or("https://api.openai.com/v1/chat/completions");

        let response = client
            .post(endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": self.config.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": prompt}
                ],
                "max_tokens": self.config.max_tokens,
                "temperature": self.config.temperature,
            }))
            .send()
            .await?;

        let response_json: serde_json::Value = response.json().await?;
        let content = response_json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?;

        // Parse the JSON response
        let parsed: serde_json::Value = serde_json::from_str(content)?;
        let events: Vec<InputEvent> = serde_json::from_value(
            parsed
                .get("events")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![])),
        )?;

        Ok(events)
    }

    fn name(&self) -> &'static str {
        "OpenAI"
    }
}

/// Claude (Anthropic) provider implementation
pub struct ClaudeProvider {
    config: LLMConfig,
}

impl ClaudeProvider {
    pub fn new(config: &LLMConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }
}

#[async_trait::async_trait]
impl LLMProvider for ClaudeProvider {
    async fn generate_workflow(
        &self,
        prompt: &str,
        _screenshot: Option<&[u8]>,
        ax_tree: Option<&str>,
        element_context: &[ElementInfo],
    ) -> anyhow::Result<Vec<InputEvent>> {
        let api_key = self
            .config
            .api_key()
            .ok_or_else(|| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

        let client = reqwest::Client::new();

        let system_prompt = format!(
            "You are an AI automation assistant. Convert natural language commands into structured input events.\
            \n\nTask: {}\
            \n\nAvailable UI elements: {:?}\
            \n\nAccessibility tree context: {:?}\
            \n\nOutput ONLY valid JSON matching this schema: {{\"events\": [INPUT_EVENTS]}}",
            prompt,
            element_context,
            ax_tree.unwrap_or("no context")
        );

        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": self.config.model,
                "messages": [
                    {"role": "user", "content": system_prompt}
                ],
                "max_tokens": self.config.max_tokens,
            }))
            .send()
            .await?;

        // Parse Claude response (similar structure but different field names)
        let response_json: serde_json::Value = response.json().await?;
        let content = response_json
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?;

        let parsed: serde_json::Value = serde_json::from_str(content)?;
        let events: Vec<InputEvent> = serde_json::from_value(
            parsed
                .get("events")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![])),
        )?;

        Ok(events)
    }

    fn name(&self) -> &'static str {
        "Claude"
    }
}

/// Local fallback provider (heuristic-based)
pub struct LocalFallback;

impl LocalFallback {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl LLMProvider for LocalFallback {
    async fn generate_workflow(
        &self,
        prompt: &str,
        _screenshot: Option<&[u8]>,
        _ax_tree: Option<&str>,
        element_context: &[ElementInfo],
    ) -> anyhow::Result<Vec<InputEvent>> {
        // Heuristic-based generation without LLM API
        let mut events = Vec::new();
        let prompt_lower = prompt.to_lowercase();

        // Simple keyword matching for common actions
        if prompt_lower.contains("click") || prompt_lower.contains("click on") {
            if let Some(element) = element_context.first() {
                events.push(InputEvent::MouseClick {
                    x: element.fallback_coords.map(|(x, _y)| x).unwrap_or(0),
                    y: element.fallback_coords.map(|(_x, y)| y).unwrap_or(0),
                    button: 0,
                    element: Some(element.clone()),
                    timestamp: None,
                    retry_count: None,
                    semantic_tag: Some(crate::core::events::SemanticTag {
                        action: "click".to_string(),
                        target: element.name.clone(),
                        confidence: 0.8,
                        ui_element: Some(element.clone()),
                        ai_generated: false,
                    }),
                    self_heal: Some(true),
                });
            }
        }

        if prompt_lower.contains("type") || prompt_lower.contains("enter") {
            events.push(InputEvent::Key {
                code: 0,
                chars: String::new(),
                modifiers: 0,
                action: crate::core::events::KeyAction::Down,
                timestamp: None,
                retry_count: None,
                semantic_tag: Some(crate::core::events::SemanticTag {
                    action: "type".to_string(),
                    target: "keyboard input".to_string(),
                    confidence: 0.7,
                    ui_element: None,
                    ai_generated: false,
                }),
            });
        }

        if prompt_lower.contains("wait") || prompt_lower.contains("delay") {
            events.push(InputEvent::Delay {
                ms: 1000,
                timestamp: None,
            });
        }

        Ok(events)
    }

    fn name(&self) -> &'static str {
        "Local Fallback"
    }
}

/// Convert events to JSON string for storage/transmission
pub fn events_to_json(events: &[InputEvent]) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(events)?)
}

/// Convert JSON string to events
pub fn json_to_events(json: &str) -> anyhow::Result<Vec<InputEvent>> {
    Ok(serde_json::from_str(json)?)
}

/// Generate a semantic description for an event
pub fn describe_event(event: &InputEvent) -> String {
    match event {
        InputEvent::MouseClick {
            x,
            y,
            button: _,
            element,
            ..
        } => {
            if let Some(el) = element {
                format!("Click {} at ({}, {}) on {}", el.role, x, y, el.name)
            } else {
                format!("Click at ({}, {})", x, y)
            }
        }
        InputEvent::Key {
            code,
            chars,
            action,
            ..
        } => {
            format!("Key {:?} {} ({})", action, chars, code)
        }
        InputEvent::Scroll { dx, dy, .. } => {
            format!("Scroll by ({}, {})", dx, dy)
        }
        InputEvent::Delay { ms, .. } => {
            format!("Wait {}ms", ms)
        }
        InputEvent::Wait {
            condition,
            timeout_ms,
            ..
        } => {
            format!(
                "Wait for condition (timeout {}ms): {:?}",
                timeout_ms, condition
            )
        }
        InputEvent::VisualCheck { threshold, .. } => {
            format!("Visual check (threshold {})", threshold)
        }
        InputEvent::Variable {
            name,
            value_template,
            ..
        } => {
            format!("Set variable {} = {}", name, value_template)
        }
        InputEvent::VariableRef { name } => {
            format!("Ref variable ${}", name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ai_settings(provider: &str, endpoint: Option<&str>) -> AISettings {
        AISettings {
            enabled: true,
            provider: provider.to_string(),
            api_endpoint: endpoint.map(|s| s.to_string()),
            model: "test-model".to_string(),
            auto_optimize: true,
            proactive_suggestions: true,
        }
    }

    #[test]
    fn local_provider_maps_to_local() {
        let cfg = LLMConfig::from_ghost_config(&ai_settings("local", None));
        assert!(matches!(cfg.provider, LLMProviderType::Local));
        // Local always uses the heuristic model name, ignoring config.model.
        assert_eq!(cfg.model, "local-heuristic");
    }

    #[test]
    fn unknown_provider_falls_back_to_local() {
        let cfg = LLMConfig::from_ghost_config(&ai_settings("wat", None));
        assert!(matches!(cfg.provider, LLMProviderType::Local));
    }

    #[test]
    fn remote_provider_without_key_falls_back_to_local() {
        // In the test environment no API keys are configured, so a requested
        // remote provider must degrade to Local rather than error later.
        if !env_key_present("OPENAI_API_KEY") {
            let cfg = LLMConfig::from_ghost_config(&ai_settings("openai", None));
            assert!(matches!(cfg.provider, LLMProviderType::Local));
        }
        if !env_key_present("ANTHROPIC_API_KEY") {
            let cfg = LLMConfig::from_ghost_config(&ai_settings("anthropic", None));
            assert!(matches!(cfg.provider, LLMProviderType::Local));
        }
    }

    #[test]
    fn endpoint_passes_through() {
        let cfg =
            LLMConfig::from_ghost_config(&ai_settings("local", Some("https://example.test/v1")));
        assert_eq!(cfg.endpoint.as_deref(), Some("https://example.test/v1"));
    }
}
