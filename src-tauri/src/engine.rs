//! Ghost engine: platform-agnostic orchestration layer.
//! Manages recording, element lookup, and replay with cancellation support.

use crate::auth::AuthManager;
use crate::config::GhostConfig;
use crate::core::ai::WorkflowAnalysis;
use crate::core::ai::WorkflowAnalyzer;
use crate::core::events::{
    ElementInfo, InputEvent, KeyAction, VisualCheckPoint, WaitCondition, Workflow, WorkflowMetadata,
};
use crate::core::execution::ExecutionHistory;
use crate::core::knowledge::{KnowledgeBase, LearnedPattern, ProactiveSuggestion};
use crate::core::llm::{self, LLMConfig};
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use crate::core::vision;
use crate::core::wait::smart_wait;
use crate::performance::{PerformanceMonitor, PerformanceSummary};
use crate::telemetry::{TelemetryManager, UsageStats};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use image::DynamicImage;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

/// RAII guard marking a replay as active for its lifetime (drop-safe, so the
/// flag clears even if the replay errors or panics).
struct ReplayActiveGuard(Arc<AtomicBool>);

impl ReplayActiveGuard {
    fn new(flag: Arc<AtomicBool>) -> Self {
        flag.store(true, Ordering::Relaxed);
        ReplayActiveGuard(flag)
    }
}

impl Drop for ReplayActiveGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// Main engine struct that holds platform-specific backends.
pub struct GhostEngine {
    recorder: Box<dyn InputRecorder>,
    locator: Box<dyn ElementLocator>,
    replayer: Box<dyn ReplayEngine>,
    /// Channel sender for recording events
    tx: Mutex<Option<mpsc::Sender<InputEvent>>>,
    /// Receiver stored for the bridge thread to consume
    rx: Mutex<Option<mpsc::Receiver<InputEvent>>>,
    /// Atomic flag for instant replay cancellation
    replay_stop_flag: Arc<AtomicBool>,
    /// True only while a replay is actually executing
    replay_active: Arc<AtomicBool>,
    /// Playback speed factor (1.0 = normal)
    playback_speed: Arc<Mutex<f32>>,
    /// Pause state for replay
    replay_paused: Arc<AtomicBool>,
    /// Recorded events buffer
    recorded_events: Arc<Mutex<Vec<InputEvent>>>,
    /// AI workflow analyzer
    analyzer: WorkflowAnalyzer,
    /// Execution history tracker
    execution_tracker: Arc<Mutex<Option<ExecutionHistory>>>,
    /// Knowledge base for Smart Observer Mode
    knowledge_base: KnowledgeBase,
    /// Persisted user configuration (source of truth for runtime defaults)
    config: Arc<Mutex<GhostConfig>>,
    /// Opt-in usage telemetry (gated by config.privacy.telemetry_enabled)
    telemetry: Arc<TelemetryManager>,
    /// Opt-in performance monitor (gated by config.performance.profiling_enabled)
    perf: Arc<PerformanceMonitor>,
    /// Wall-clock start of the active recording session, for duration telemetry
    recording_start: Arc<Mutex<Option<Instant>>>,
    /// Local login + at-rest encryption for workflow data
    auth: Arc<AuthManager>,
}

impl GhostEngine {
    /// Create a new GhostEngine with the appropriate platform backend.
    pub fn new() -> Self {
        #[cfg(target_os = "macos")]
        let (recorder, locator, replayer) = {
            use crate::platform::macos::MacosBackend;
            (
                MacosBackend::recorder(),
                MacosBackend::locator(),
                MacosBackend::replayer(),
            )
        };

        #[cfg(target_os = "windows")]
        let (recorder, locator, replayer) = {
            use crate::platform::windows::WindowsBackend;
            (
                WindowsBackend::recorder(),
                WindowsBackend::locator(),
                WindowsBackend::replayer(),
            )
        };

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        compile_error!("Unsupported platform: only macOS and Windows are supported");

        // Load persisted config (falling back to defaults) and use it to seed
        // runtime state: starting playback speed and the active LLM provider.
        let config = GhostConfig::load().unwrap_or_default();
        let initial_speed = config.replay.default_speed.max(0.1);
        llm::init_llm(&LLMConfig::from_ghost_config(&config.ai));

        // Observability is opt-in: both honor the persisted privacy/performance
        // flags and no-op until enabled, so default runs collect nothing.
        let telemetry = Arc::new(TelemetryManager::new(config.privacy.telemetry_enabled));
        let perf = Arc::new(PerformanceMonitor::new(
            config.performance.profiling_enabled,
        ));

        GhostEngine {
            recorder,
            locator,
            replayer,
            tx: Mutex::new(None),
            rx: Mutex::new(None),
            replay_stop_flag: Arc::new(AtomicBool::new(false)),
            replay_active: Arc::new(AtomicBool::new(false)),
            playback_speed: Arc::new(Mutex::new(initial_speed)),
            replay_paused: Arc::new(AtomicBool::new(false)),
            recorded_events: Arc::new(Mutex::new(Vec::new())),
            analyzer: WorkflowAnalyzer::new(),
            execution_tracker: Arc::new(Mutex::new(ExecutionHistory::new().ok())),
            knowledge_base: KnowledgeBase::new(),
            config: Arc::new(Mutex::new(config)),
            telemetry,
            perf,
            recording_start: Arc::new(Mutex::new(None)),
            auth: Arc::new(AuthManager::new()),
        }
    }

    /// Access the local auth manager (login + workflow encryption).
    pub fn auth(&self) -> Arc<AuthManager> {
        Arc::clone(&self.auth)
    }

    /// Start recording input events. Events will be sent through the provided channel.
    pub fn start_recording(&self, tx: mpsc::Sender<InputEvent>) -> anyhow::Result<()> {
        // Clear previous recorded events
        *self.recorded_events.lock().unwrap() = Vec::new();

        // Store the sender and receiver for later use
        let (tx_clone, rx) = mpsc::channel();
        *self.tx.lock().unwrap() = Some(tx_clone);
        *self.rx.lock().unwrap() = Some(rx);

        // Mark the session start so stop_recording can report its duration.
        *self.recording_start.lock().unwrap() = Some(Instant::now());

        self.recorder.start(tx)
    }

    /// Stop the active recording session.
    pub fn stop_recording(&self) {
        self.recorder.stop();
        *self.tx.lock().unwrap() = None;
        *self.rx.lock().unwrap() = None;

        // Report the completed recording to telemetry (no-op unless opted in).
        if let Some(started) = self.recording_start.lock().unwrap().take() {
            let event_count = self.recorded_events.lock().unwrap().len();
            self.telemetry
                .track_workflow_recorded(event_count, started.elapsed().as_secs());
        }
    }

    /// Add an event to the recorded events buffer (called from the bridge thread)
    pub fn buffer_event(&self, event: InputEvent) {
        self.recorded_events.lock().unwrap().push(event);
    }

    /// Get all recorded events
    pub fn get_recorded_events(&self) -> Vec<InputEvent> {
        self.recorded_events.lock().unwrap().clone()
    }

    /// Replay a sequence of recorded events.
    pub fn replay(&self, events: &[InputEvent]) -> anyhow::Result<()> {
        // Reset the stop flag and pause state before starting
        self.replay_stop_flag.store(false, Ordering::Relaxed);
        self.replay_paused.store(false, Ordering::Relaxed);

        // Time and record the replay (both no-op unless the user opted in).
        let started = Instant::now();
        self.perf.start_timer("replay");
        let _active = ReplayActiveGuard::new(self.replay_active.clone());
        let result = self.replayer.execute(
            events,
            self.replay_stop_flag.clone(),
            self.replay_paused.clone(),
            self.get_playback_speed(),
        );
        drop(_active);
        self.perf.stop_timer("replay");
        self.telemetry.track_workflow_replayed(
            events.len(),
            started.elapsed().as_secs(),
            result.is_ok(),
        );

        result
    }

    /// Cancel an ongoing replay immediately.
    pub fn cancel_replay(&self) {
        self.replay_stop_flag.store(true, Ordering::Relaxed);
    }

    /// Pause an ongoing replay.
    pub fn pause_replay(&self) {
        self.replay_paused.store(true, Ordering::Relaxed);
    }

    /// Resume a paused replay.
    pub fn resume_replay(&self) {
        self.replay_paused.store(false, Ordering::Relaxed);
    }

    /// Check if replay is currently paused.
    pub fn is_replay_paused(&self) -> bool {
        self.replay_paused.load(Ordering::Relaxed)
    }

    /// Set the playback speed factor.
    pub fn set_playback_speed(&self, factor: f32) {
        *self.playback_speed.lock().unwrap() = factor.max(0.1);
    }

    /// Get the current playback speed factor.
    pub fn get_playback_speed(&self) -> f32 {
        *self.playback_speed.lock().unwrap()
    }

    /// Snapshot the current persisted configuration.
    pub fn get_config(&self) -> GhostConfig {
        self.config.lock().unwrap().clone()
    }

    /// Validate, persist, and apply a new configuration. Re-seeds the live
    /// playback speed and rebuilds the active LLM provider so changes take
    /// effect without a restart.
    pub fn update_config(&self, new_config: GhostConfig) -> anyhow::Result<()> {
        new_config.validate()?;
        new_config.save()?;

        *self.playback_speed.lock().unwrap() = new_config.replay.default_speed.max(0.1);
        llm::init_llm(&LLMConfig::from_ghost_config(&new_config.ai));

        // Honor opt-in toggles live, mirroring the playback-speed/LLM re-seed above.
        self.telemetry
            .set_enabled(new_config.privacy.telemetry_enabled);
        self.perf
            .set_enabled(new_config.performance.profiling_enabled);

        *self.config.lock().unwrap() = new_config;
        Ok(())
    }

    /// Snapshot the collected usage telemetry statistics.
    pub fn get_telemetry_stats(&self) -> UsageStats {
        self.telemetry.get_stats()
    }

    /// Export all collected telemetry (session id, stats, events) as JSON.
    pub fn export_telemetry(&self) -> anyhow::Result<String> {
        Ok(self.telemetry.export_json()?)
    }

    /// Summarize recorded performance metrics by operation.
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        self.perf.get_summary()
    }

    /// Record a feature-usage event (no-op unless telemetry is enabled).
    pub fn track_feature(&self, feature: &str) {
        self.telemetry.track_feature_used(feature);
    }

    /// Build a default retry config from the persisted replay settings.
    pub fn default_retry_config(&self) -> crate::core::events::RetryConfig {
        let replay = &self.config.lock().unwrap().replay;
        crate::core::events::RetryConfig {
            max_attempts: replay.max_retry_attempts,
            backoff_ms: replay.retry_backoff_ms,
            backoff_multiplier: replay.retry_backoff_multiplier,
        }
    }

    /// Get the element info at the given screen coordinates.
    pub fn inspect_element(
        &self,
        x: i32,
        y: i32,
    ) -> anyhow::Result<Option<crate::core::events::ElementInfo>> {
        self.locator.inspect_at(x, y)
    }

    /// Get a clone of the replay stop flag for external monitoring.
    pub fn get_stop_flag(&self) -> Arc<AtomicBool> {
        self.replay_stop_flag.clone()
    }

    /// Save workflow to a JSON file in the app's data directory.
    pub fn save_workflow(&self, name: &str, events: &[InputEvent]) -> anyhow::Result<PathBuf> {
        use std::fs;

        // Get the data directory
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let workflows_dir = data_dir.join("ghost").join("workflows");
        fs::create_dir_all(&workflows_dir)?;

        let file_path = workflows_dir.join(format!("{}.json", name));
        let json = serde_json::to_string_pretty(events)?;
        // Encrypted at rest when a local password is configured (auth.rs).
        fs::write(&file_path, self.auth.protect(&json)?)?;

        Ok(file_path)
    }

    /// Load workflow from a JSON file in the app's data directory.
    pub fn load_workflow(&self, name: &str) -> anyhow::Result<Vec<InputEvent>> {
        use std::fs;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let file_path = data_dir
            .join("ghost")
            .join("workflows")
            .join(format!("{}.json", name));
        // Transparently decrypts envelopes; pre-password plaintext loads as-is.
        let json = self.auth.reveal(&fs::read_to_string(&file_path)?)?;
        let events: Vec<InputEvent> = serde_json::from_str(&json)?;

        Ok(events)
    }

    /// Delete a workflow from disk.
    pub fn delete_workflow(&self, name: &str) -> anyhow::Result<()> {
        use std::fs;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let file_path = data_dir
            .join("ghost")
            .join("workflows")
            .join(format!("{}.json", name));

        if file_path.exists() {
            fs::remove_file(file_path)?;
        }

        Ok(())
    }

    /// List all saved workflows.
    pub fn list_workflows() -> anyhow::Result<Vec<String>> {
        use std::fs;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let workflows_dir = data_dir.join("ghost").join("workflows");

        if !workflows_dir.exists() {
            return Ok(Vec::new());
        }

        let mut workflows = Vec::new();
        for entry in fs::read_dir(workflows_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    workflows.push(name.to_string());
                }
            }
        }

        Ok(workflows)
    }

    /// Analyze the current workflow and return AI-powered insights
    pub fn analyze_workflow(&self, events: &[InputEvent], name: &str) -> WorkflowAnalysis {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let metadata = WorkflowMetadata {
            name: name.to_string(),
            description: String::new(),
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            estimated_duration_ms: events
                .iter()
                .filter_map(|e| {
                    if let InputEvent::Delay { ms, .. } = e {
                        Some(*ms)
                    } else {
                        None
                    }
                })
                .sum(),
            reliability_score: 1.0,
            element_confidence: 1.0,
        };

        self.analyzer.analyze(events, &metadata)
    }

    /// Generate a workflow object with metadata
    pub fn create_workflow(&self, name: &str, events: &[InputEvent]) -> Workflow {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Workflow {
            name: name.to_string(),
            events: events.to_vec(),
            metadata: WorkflowMetadata {
                name: name.to_string(),
                description: format!(
                    "Automatically generated workflow with {} events",
                    events.len()
                ),
                tags: Vec::new(),
                created_at: now,
                updated_at: now,
                estimated_duration_ms: events
                    .iter()
                    .filter_map(|e| {
                        if let InputEvent::Delay { ms, .. } = e {
                            Some(*ms)
                        } else {
                            None
                        }
                    })
                    .sum(),
                reliability_score: self.analyzer.calculate_reliability(events),
                element_confidence: self.analyzer.calculate_element_richness(events),
            },
            reliability: None,
        }
    }

    /// Save a complete workflow with metadata
    pub fn save_workflow_with_metadata(&self, workflow: &Workflow) -> anyhow::Result<PathBuf> {
        use std::fs;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let workflows_dir = data_dir.join("ghost").join("workflows");
        fs::create_dir_all(&workflows_dir)?;

        let file_path = workflows_dir.join(format!("{}.json", workflow.name));
        let json = serde_json::to_string_pretty(workflow)?;
        fs::write(&file_path, self.auth.protect(&json)?)?;

        Ok(file_path)
    }

    /// Save a workflow with custom description and tags
    pub fn save_workflow_with_details(
        &self,
        name: &str,
        events: &[InputEvent],
        description: &str,
        tags: &[String],
    ) -> anyhow::Result<PathBuf> {
        let workflow = self.create_workflow_with_details(name, events, description, tags);
        self.save_workflow_with_metadata(&workflow)
    }

    /// Create a workflow with custom metadata
    pub fn create_workflow_with_details(
        &self,
        name: &str,
        events: &[InputEvent],
        description: &str,
        tags: &[String],
    ) -> Workflow {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Workflow {
            name: name.to_string(),
            events: events.to_vec(),
            metadata: WorkflowMetadata {
                name: name.to_string(),
                description: description.to_string(),
                tags: tags.to_vec(),
                created_at: now,
                updated_at: now,
                estimated_duration_ms: events
                    .iter()
                    .filter_map(|e| {
                        if let InputEvent::Delay { ms, .. } = e {
                            Some(*ms)
                        } else {
                            None
                        }
                    })
                    .sum(),
                reliability_score: self.analyzer.calculate_reliability(events),
                element_confidence: self.analyzer.calculate_element_richness(events),
            },
            reliability: None,
        }
    }

    /// Generate a workflow name suggestion based on the events
    pub fn generate_workflow_name(&self, events: &[InputEvent]) -> anyhow::Result<String> {
        Ok(self.analyzer.generate_workflow_name(events))
    }

    /// Load a complete workflow with metadata
    pub fn load_workflow_with_metadata(&self, name: &str) -> anyhow::Result<Workflow> {
        use std::fs;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let file_path = data_dir
            .join("ghost")
            .join("workflows")
            .join(format!("{}.json", name));
        let json = self.auth.reveal(&fs::read_to_string(&file_path)?)?;
        let workflow: Workflow = serde_json::from_str(&json)?;

        Ok(workflow)
    }

    /// Replay a workflow with reliability features
    pub fn replay_with_reliability(
        &self,
        events: &[InputEvent],
        reliability: &crate::core::events::ReliabilitySettings,
    ) -> anyhow::Result<()> {
        // Reset flags
        self.replay_stop_flag.store(false, Ordering::Relaxed);
        self.replay_paused.store(false, Ordering::Relaxed);

        let _active = ReplayActiveGuard::new(self.replay_active.clone());
        self.replayer.execute_with_reliability(
            events,
            self.replay_stop_flag.clone(),
            self.replay_paused.clone(),
            self.get_playback_speed(),
            reliability,
        )
    }

    /// Get element info at coordinates for validation
    pub fn validate_element_at(&self, x: i32, y: i32) -> anyhow::Result<bool> {
        Ok(self.locator.inspect_at(x, y)?.is_some())
    }

    /// Check if replay is currently running
    pub fn is_replay_running(&self) -> bool {
        self.replay_active.load(Ordering::Relaxed)
    }

    /// Generate workflow from natural language prompt using LLM
    pub fn generate_workflow_from_prompt(
        &self,
        prompt: String,
        screenshot: Option<Vec<u8>>,
    ) -> anyhow::Result<Vec<InputEvent>> {
        // Initialize the LLM from the persisted config if not already done
        // (it normally is, from `new()`/`update_config`).
        if llm::get_llm().is_none() {
            let ai = self.config.lock().unwrap().ai.clone();
            llm::init_llm(&LLMConfig::from_ghost_config(&ai));
        }

        let provider =
            llm::get_llm().ok_or_else(|| anyhow::anyhow!("No LLM provider available"))?;

        // Get element context from current screen
        let element_context = self.get_visible_elements()?;

        // Call the LLM (async, but we'll block on it for Tauri command)
        let rt = tokio::runtime::Runtime::new()?;
        let events = rt.block_on(async {
            provider
                .generate_workflow(
                    &prompt,
                    screenshot.as_deref(),
                    None, // AX tree would be populated here
                    &element_context,
                )
                .await
        })?;

        Ok(events)
    }

    /// Get visible elements for context
    fn get_visible_elements(&self) -> anyhow::Result<Vec<ElementInfo>> {
        let mut elements = Vec::new();

        // Probe a coarse grid — elements are tens of pixels wide, so a 48px
        // stride captures them with ~700 lookups instead of the 250k a
        // per-pixel scan would take (minutes of AX traffic per call).
        const STRIDE: usize = 48;
        for y in (0..1080).step_by(STRIDE) {
            for x in (0..1600).step_by(STRIDE) {
                if let Ok(Some(el)) = self.locator.inspect_at(x, y) {
                    // Avoid duplicates
                    if !elements
                        .iter()
                        .any(|e: &ElementInfo| e.name == el.name && e.role == el.role)
                    {
                        elements.push(el);
                    }
                }
            }
        }

        Ok(elements)
    }

    /// Analyze and add semantic tags to recorded events
    pub fn analyze_and_tag_workflow(
        &self,
        events: Vec<InputEvent>,
    ) -> anyhow::Result<Vec<InputEvent>> {
        if llm::get_llm().is_none() {
            let ai = self.config.lock().unwrap().ai.clone();
            llm::init_llm(&LLMConfig::from_ghost_config(&ai));
        }

        let _provider =
            llm::get_llm().ok_or_else(|| anyhow::anyhow!("No AI provider available"))?;

        let element_context = self.get_visible_elements()?;

        let rt = tokio::runtime::Runtime::new()?;
        let tagged_events = rt.block_on(async {
            // Use the analyzer for simpler heuristic-based tagging
            // LLM-based tagging would involve sending the full event stream
            let metadata = WorkflowMetadata::default();
            let _analysis = self.analyzer.analyze(&events, &metadata);

            // For each event, add semantic context
            let mut result = Vec::new();
            for event in events {
                let tagged = self.add_semantic_context(&event, &element_context);
                result.push(tagged);
            }
            result
        });

        Ok(tagged_events)
    }

    /// Add semantic context to an event. Preserves the recorded timestamp —
    /// replay pacing depends on it surviving the tagging pass.
    fn add_semantic_context(&self, event: &InputEvent, elements: &[ElementInfo]) -> InputEvent {
        match event {
            InputEvent::MouseClick {
                x,
                y,
                button,
                element,
                timestamp,
                ..
            } => {
                let semantic_tag = element
                    .clone()
                    .or_else(|| self.find_closest_element(*x, *y, elements))
                    .map(|el| crate::core::events::SemanticTag {
                        action: "click".to_string(),
                        target: el.name.clone(),
                        confidence: 0.95,
                        ui_element: Some(el.clone()),
                        ai_generated: false,
                    });

                InputEvent::MouseClick {
                    x: *x,
                    y: *y,
                    button: *button,
                    element: element.clone(),
                    timestamp: *timestamp,
                    retry_count: None,
                    semantic_tag,
                    self_heal: Some(true),
                }
            }
            InputEvent::Key {
                code,
                chars,
                modifiers,
                action,
                timestamp,
                ..
            } => {
                let semantic_tag = if !chars.is_empty() {
                    Some(crate::core::events::SemanticTag {
                        action: "type".to_string(),
                        target: format!("Keyboard input: {}", chars),
                        confidence: 0.9,
                        ui_element: None,
                        ai_generated: false,
                    })
                } else {
                    None
                };

                InputEvent::Key {
                    code: *code,
                    chars: chars.clone(),
                    modifiers: *modifiers,
                    action: action.clone(),
                    timestamp: *timestamp,
                    retry_count: None,
                    semantic_tag,
                }
            }
            other => other.clone(),
        }
    }

    /// Find the closest element to given coordinates
    fn find_closest_element(
        &self,
        x: i32,
        y: i32,
        elements: &[ElementInfo],
    ) -> Option<ElementInfo> {
        elements
            .iter()
            .filter_map(|el| {
                el.fallback_coords.as_ref().map(|(ex, ey)| {
                    let dist = ((x - ex).pow(2) + (y - ey).pow(2)) as f32;
                    (el, dist)
                })
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(el, _)| el.clone())
    }

    /// Wait for a condition during workflow execution
    pub fn wait_for_condition(
        &self,
        condition: &WaitCondition,
        timeout_ms: u64,
        poll_interval_ms: u64,
    ) -> anyhow::Result<()> {
        smart_wait(
            condition,
            self.locator.as_ref(),
            timeout_ms,
            poll_interval_ms,
        )
        .map_err(|e| anyhow::anyhow!("Wait failed: {}", e))
    }

    /// Perform visual regression check
    pub fn check_visual_regression(
        &self,
        baseline_path: &str,
        current: &DynamicImage,
        threshold: f32,
    ) -> anyhow::Result<bool> {
        let similarity = vision::compare_images(baseline_path, current)?;
        Ok(similarity >= threshold)
    }

    /// Save a screenshot to disk
    pub fn save_screenshot(&self, img: &[u8], path: &str) -> anyhow::Result<()> {
        let dynamic_image = image::load_from_memory(img)?;
        vision::save_image(&dynamic_image, path)?;
        Ok(())
    }

    // ===== Phase 4A: Visual Regression Replay =====

    /// Replay with visual regression checkpoints
    pub fn replay_with_visual_check(
        &self,
        events: &[InputEvent],
        visual_checkpoints: &[VisualCheckPoint],
    ) -> anyhow::Result<bool> {
        // Reset flags
        self.replay_stop_flag.store(false, Ordering::Relaxed);
        self.replay_paused.store(false, Ordering::Relaxed);

        use crate::core::replay_support::{check_continue, interruptible_sleep, pacing_gap_ms};

        let _active = ReplayActiveGuard::new(self.replay_active.clone());
        let mut enigo = Enigo::new(&Settings::default())?;
        let speed = (*self.playback_speed.lock().unwrap()).max(0.1);
        let mut prev_ts: Option<u64> = None;

        for (idx, event) in events.iter().enumerate() {
            if !check_continue(&self.replay_stop_flag, &self.replay_paused) {
                return Ok(false);
            }

            // Reproduce the recorded rhythm between events.
            let gap = pacing_gap_ms(prev_ts, event.timestamp());
            if gap > 0
                && !interruptible_sleep(
                    (gap as f32 / speed) as u64,
                    &self.replay_stop_flag,
                    &self.replay_paused,
                )
            {
                return Ok(false);
            }
            if let Some(ts) = event.timestamp() {
                prev_ts = Some(ts);
            }

            // Check if we need to perform a visual check at this index
            let checkpoint = visual_checkpoints.iter().find(|c| c.event_index == idx);

            // Execute the event
            match event {
                InputEvent::MouseClick { x, y, button, .. } => {
                    // Mirror recorded press/release (0/2 = down, 1/3 = up) so
                    // clicks don't double-fire and drags survive.
                    let (mouse_button, direction) = match button {
                        0 => (Button::Left, Direction::Press),
                        1 => (Button::Left, Direction::Release),
                        2 => (Button::Right, Direction::Press),
                        3 => (Button::Right, Direction::Release),
                        _ => (Button::Left, Direction::Click),
                    };
                    enigo.move_mouse(*x, *y, Coordinate::Abs)?;
                    enigo.button(mouse_button, direction)?;
                }
                InputEvent::Key {
                    code,
                    chars,
                    action,
                    ..
                } => {
                    let key = if !chars.is_empty() {
                        Key::Unicode(chars.chars().next().unwrap_or(' '))
                    } else {
                        Key::Other(*code as u32)
                    };
                    match action {
                        KeyAction::Down => {
                            enigo.key(key, Direction::Press)?;
                        }
                        KeyAction::Up => {
                            enigo.key(key, Direction::Release)?;
                        }
                    }
                }
                InputEvent::Scroll { dx, dy, .. } => {
                    if *dx != 0 {
                        enigo.scroll(*dx, Axis::Horizontal)?;
                    }
                    if *dy != 0 {
                        enigo.scroll(*dy, Axis::Vertical)?;
                    }
                }
                InputEvent::Delay { ms, .. } => {
                    let adjusted_ms = (*ms as f32 / speed) as u64;
                    if !interruptible_sleep(
                        adjusted_ms,
                        &self.replay_stop_flag,
                        &self.replay_paused,
                    ) {
                        return Ok(false);
                    }
                }
                _ => {}
            }

            // Perform visual check if configured
            if let Some(checkpoint) = checkpoint {
                if let Some(baseline_path) = &checkpoint.baseline_screenshot_path {
                    if let Ok(img_bytes) = vision::capture_screenshot() {
                        if let Ok(current_img) = image::load_from_memory(&img_bytes) {
                            let similarity =
                                vision::compare_images(baseline_path, &current_img).unwrap_or(1.0);

                            if similarity < checkpoint.threshold {
                                tracing::warn!(
                                    "Visual check '{}' failed: {:.2} < {}",
                                    checkpoint.name,
                                    similarity,
                                    checkpoint.threshold
                                );
                                // Continue anyway - could be made configurable
                            } else {
                                tracing::info!(
                                    "Visual check '{}' passed: {:.2}",
                                    checkpoint.name,
                                    similarity
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(true)
    }

    /// Capture and save a baseline screenshot
    pub fn capture_baseline(
        &self,
        name: &str,
        _region: Option<(i32, i32, i32, i32)>,
    ) -> anyhow::Result<String> {
        let img_bytes = vision::capture_screenshot()
            .map_err(|e| anyhow::anyhow!("Failed to capture screenshot: {}", e))?;

        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let baselines_dir = data_dir.join("ghost").join("baselines");
        std::fs::create_dir_all(&baselines_dir)?;

        let path = baselines_dir.join(format!("{}.png", name));
        self.save_screenshot(&img_bytes, path.to_string_lossy().as_ref())?;

        Ok(path.to_string_lossy().to_string())
    }

    // ===== Phase 4C: Data Source Management =====

    /// Create a data source for variable-driven workflows
    pub fn create_data_source(
        &self,
        name: &str,
        source_type: &str,
        path: Option<&str>,
    ) -> anyhow::Result<String> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let sources_dir = data_dir.join("ghost").join("data_sources");
        std::fs::create_dir_all(&sources_dir)?;

        let source_path = match source_type {
            "csv" | "json" => {
                let p = path.ok_or_else(|| {
                    anyhow::anyhow!("Path required for {} data source", source_type)
                })?;
                format!("{}:{}", source_type, p)
            }
            "environment" => "environment".to_string(),
            _ => return Err(anyhow::anyhow!("Unknown source type: {}", source_type)),
        };

        let file_path = sources_dir.join(format!("{}.json", name));
        let metadata = serde_json::json!({
            "name": name,
            "type": source_type,
            "path": source_path,
            "created_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });

        std::fs::write(&file_path, serde_json::to_string_pretty(&metadata)?)?;
        Ok(file_path.to_string_lossy().to_string())
    }

    /// Load variables from a data source
    pub fn load_variables(
        &self,
        data_source_name: &str,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let sources_dir = data_dir.join("ghost").join("data_sources");
        let file_path = sources_dir.join(format!("{}.json", data_source_name));

        let json = std::fs::read_to_string(&file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read data source: {}", e))?;

        let metadata: serde_json::Value = serde_json::from_str(&json)?;
        let source_type = metadata["type"].as_str().unwrap_or("unknown");

        let mut variables = std::collections::HashMap::new();

        match source_type {
            "csv" => {
                let path = metadata["path"]
                    .as_str()
                    .and_then(|p| p.strip_prefix("csv:"))
                    .ok_or_else(|| anyhow::anyhow!("Invalid CSV path in data source"))?;

                let csv_content = std::fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("Failed to read CSV file: {}", e))?;

                // Parse CSV and extract first row as variables
                for line in csv_content.lines() {
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() >= 2 {
                        variables.insert(parts[0].to_string(), parts[1].to_string());
                    }
                }
            }
            "json" => {
                let path = metadata["path"]
                    .as_str()
                    .and_then(|p| p.strip_prefix("json:"))
                    .ok_or_else(|| anyhow::anyhow!("Invalid JSON path in data source"))?;

                let json_content = std::fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("Failed to read JSON file: {}", e))?;

                let json_vars: serde_json::Value = serde_json::from_str(&json_content)?;
                if let Some(obj) = json_vars.as_object() {
                    for (k, v) in obj {
                        variables.insert(k.clone(), v.as_str().unwrap_or_default().to_string());
                    }
                }
            }
            "environment" => {
                // Load from environment variables
                for (key, value) in std::env::vars() {
                    variables.insert(key, value);
                }
            }
            _ => return Err(anyhow::anyhow!("Unknown source type: {}", source_type)),
        }

        Ok(variables)
    }

    // ===== Smart Observer Mode Methods =====

    /// Start the Smart Observer - watch and learn user patterns
    pub fn start_observer(&self) {
        self.knowledge_base.start_observer();
    }

    /// Stop the Smart Observer
    pub fn stop_observer(&self) {
        self.knowledge_base.stop_observer();
    }

    /// Check if observer is active
    pub fn is_observer_active(&self) -> bool {
        self.knowledge_base.is_observer_active()
    }

    /// Set observer interval in milliseconds
    pub fn set_observer_interval(&self, interval_ms: u64) {
        self.knowledge_base.set_observer_interval(interval_ms);
    }

    /// Record events as an observed pattern
    pub fn observe_events(&self, events: &[InputEvent], app_name: &str) {
        let patterns = self
            .knowledge_base
            .analyze_observed_events(events, app_name);
        for pattern in patterns {
            self.knowledge_base.observe_pattern(pattern);
        }
        self.knowledge_base.track_app_usage(app_name);
    }

    /// Get proactive automation suggestions
    pub fn get_proactive_suggestions(&self) -> Vec<ProactiveSuggestion> {
        self.knowledge_base.get_suggestions()
    }

    /// Get learned patterns for an app
    pub fn get_learned_patterns(&self, app_name: Option<&str>) -> Vec<LearnedPattern> {
        match app_name {
            Some(name) => self.knowledge_base.get_app_patterns(name),
            None => self.knowledge_base.get_patterns(),
        }
    }

    /// Get app usage statistics
    pub fn get_app_usage_stats(&self) -> Vec<crate::core::knowledge::AppUsageStats> {
        self.knowledge_base.get_app_usage()
    }

    /// Get execution tracker reference
    pub fn get_execution_tracker(
        &self,
    ) -> Option<std::sync::MutexGuard<'_, Option<ExecutionHistory>>> {
        self.execution_tracker.lock().ok()
    }

    /// Generate a "geek mode" insight for events
    pub fn generate_geek_insights(
        &self,
        events: &[InputEvent],
        _app_name: &str,
    ) -> crate::core::knowledge::GeekDetails {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let timings: Vec<_> = events
            .iter()
            .enumerate()
            .map(|(idx, _)| crate::core::knowledge::EventTiming {
                event_index: idx,
                timestamp_ms: now,
                delay_before_ms: 0,
                estimated_action: "pending analysis".to_string(),
            })
            .collect();

        let total_ms: u64 = events
            .iter()
            .filter_map(|e| match e {
                InputEvent::Delay { ms, .. } => Some(*ms),
                _ => None,
            })
            .sum();

        crate::core::knowledge::GeekDetails {
            event_timing_analysis: timings,
            system_calls_traced: vec!["mouse_event".to_string(), "key_event".to_string()],
            alternative_shortcuts: vec![],
            performance_metrics: crate::core::knowledge::PerformanceMetrics {
                total_duration_ms: total_ms,
                avg_delay_ms: total_ms as f64 / events.len().max(1) as f64,
                bottleneck_events: vec![],
            },
            raw_ax_tree_snapshots: vec![],
        }
    }
}

impl Default for GhostEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_off_by_default_collects_nothing() {
        let engine = GhostEngine::new();
        // A fresh engine inherits the persisted privacy flag (false by default),
        // so feature tracking must be a no-op until explicitly opted in.
        engine.track_feature("analyze_workflow");
        let stats = engine.get_telemetry_stats();
        assert!(stats.feature_usage.is_empty());
    }

    #[test]
    fn enabling_telemetry_records_feature_usage() {
        let engine = GhostEngine::new();
        engine.telemetry.set_enabled(true);

        engine.track_feature("analyze_workflow");
        engine.track_feature("analyze_workflow");
        engine.track_feature("optimize_workflow");

        let stats = engine.get_telemetry_stats();
        assert_eq!(stats.feature_usage.get("analyze_workflow"), Some(&2));
        assert_eq!(stats.feature_usage.get("optimize_workflow"), Some(&1));

        // Export should round-trip the collected data as JSON.
        let json = engine.export_telemetry().expect("export should succeed");
        assert!(json.contains("analyze_workflow"));
    }
}
