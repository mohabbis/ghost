//! Ghost engine: platform-agnostic orchestration layer.
//! Manages recording, element lookup, and replay with cancellation support.

use crate::core::events::{ElementInfo, InputEvent, Workflow, WorkflowAnalysis, WorkflowMetadata, WaitCondition, ElementSelector};
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use crate::core::ai::WorkflowAnalyzer;
use crate::core::llm::{self, LLMConfig, LLMProvider};
use crate::core::wait::{smart_wait, WaitResult};
use crate::core::vision;
use image::DynamicImage;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::path::PathBuf;

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
    /// Playback speed factor (1.0 = normal)
    playback_speed: Arc<Mutex<f32>>,
    /// Pause state for replay
    replay_paused: Arc<AtomicBool>,
    /// Recorded events buffer
    recorded_events: Arc<Mutex<Vec<InputEvent>>>,
    /// AI workflow analyzer
    analyzer: WorkflowAnalyzer,
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

        GhostEngine {
            recorder,
            locator,
            replayer,
            tx: Mutex::new(None),
            rx: Mutex::new(None),
            replay_stop_flag: Arc::new(AtomicBool::new(false)),
            playback_speed: Arc::new(Mutex::new(1.0)),
            replay_paused: Arc::new(AtomicBool::new(false)),
            recorded_events: Arc::new(Mutex::new(Vec::new())),
            analyzer: WorkflowAnalyzer::new(),
        }
    }

    /// Start recording input events. Events will be sent through the provided channel.
    pub fn start_recording(&self, tx: mpsc::Sender<InputEvent>) -> anyhow::Result<()> {
        // Clear previous recorded events
        *self.recorded_events.lock().unwrap() = Vec::new();
        
        // Store the sender and receiver for later use
        let (tx_clone, rx) = mpsc::channel();
        *self.tx.lock().unwrap() = Some(tx_clone);
        *self.rx.lock().unwrap() = Some(rx);

        self.recorder.start(tx)
    }

    /// Stop the active recording session.
    pub fn stop_recording(&self) {
        self.recorder.stop();
        *self.tx.lock().unwrap() = None;
        *self.rx.lock().unwrap() = None;
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
        self.replayer.execute(events, self.replay_stop_flag.clone())
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

    /// Get the element info at the given screen coordinates.
    pub fn inspect_element(&self, x: i32, y: i32) -> anyhow::Result<Option<crate::core::events::ElementInfo>> {
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
        let data_dir = tauri::api::path::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        
        let workflows_dir = data_dir.join("ghost").join("workflows");
        fs::create_dir_all(&workflows_dir)?;
        
        let file_path = workflows_dir.join(format!("{}.json", name));
        let json = serde_json::to_string_pretty(events)?;
        fs::write(&file_path, json)?;
        
        Ok(file_path)
    }

    /// Load workflow from a JSON file in the app's data directory.
    pub fn load_workflow(&self, name: &str) -> anyhow::Result<Vec<InputEvent>> {
        use std::fs;
        
        let data_dir = tauri::api::path::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        
        let file_path = data_dir.join("ghost").join("workflows").join(format!("{}.json", name));
        let json = fs::read_to_string(&file_path)?;
        let events: Vec<InputEvent> = serde_json::from_str(&json)?;
        
        Ok(events)
    }

    /// Delete a workflow from disk.
    pub fn delete_workflow(&self, name: &str) -> anyhow::Result<()> {
        use std::fs;
        
        let data_dir = tauri::api::path::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        
        let file_path = data_dir.join("ghost").join("workflows").join(format!("{}.json", name));
        
        if file_path.exists() {
            fs::remove_file(file_path)?;
        }
        
        Ok(())
    }

    /// List all saved workflows.
    pub fn list_workflows() -> anyhow::Result<Vec<String>> {
        use std::fs;
        
        let data_dir = tauri::api::path::data_dir()
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
    pub fn analyze_workflow(&self, events: &[InputEvent], name: &str) -> crate::core::events::WorkflowAnalysis {
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
            estimated_duration_ms: events.iter().filter_map(|e| {
                if let InputEvent::Delay { ms, .. } = e { Some(*ms) } else { None }
            }).sum(),
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
                description: format!("Automatically generated workflow with {} events", events.len()),
                tags: Vec::new(),
                created_at: now,
                updated_at: now,
                estimated_duration_ms: events.iter().filter_map(|e| {
                    if let InputEvent::Delay { ms, .. } = e { Some(*ms) } else { None }
                }).sum(),
                reliability_score: self.analyzer.calculate_reliability(events),
                element_confidence: self.analyzer.calculate_element_richness(events),
            },
        }
    }

    /// Save a complete workflow with metadata
    pub fn save_workflow_with_metadata(&self, workflow: &Workflow) -> anyhow::Result<PathBuf> {
        use std::fs;
        
        let data_dir = tauri::api::path::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        
        let workflows_dir = data_dir.join("ghost").join("workflows");
        fs::create_dir_all(&workflows_dir)?;
        
        let file_path = workflows_dir.join(format!("{}.json", workflow.name));
        let json = serde_json::to_string_pretty(workflow)?;
        fs::write(&file_path, json)?;
        
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
                estimated_duration_ms: events.iter().filter_map(|e| {
                    if let InputEvent::Delay { ms, .. } = e { Some(*ms) } else { None }
                }).sum(),
                reliability_score: self.analyzer.calculate_reliability(events),
                element_confidence: self.analyzer.calculate_element_richness(events),
            },
        }
    }

    /// Generate a workflow name suggestion based on the events
    pub fn generate_workflow_name(&self, events: &[InputEvent]) -> anyhow::Result<String> {
        Ok(self.analyzer.generate_workflow_name(events))
    }

    /// Load a complete workflow with metadata
    pub fn load_workflow_with_metadata(&self, name: &str) -> anyhow::Result<Workflow> {
        use std::fs;
        
        let data_dir = tauri::api::path::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        
        let file_path = data_dir.join("ghost").join("workflows").join(format!("{}.json", name));
        let json = fs::read_to_string(&file_path)?;
        let workflow: Workflow = serde_json::from_str(&json)?;
        
        Ok(workflow)
    }

    /// Replay a workflow with reliability features
    pub fn replay_with_reliability(
        &self, 
        events: &[InputEvent], 
        reliability: &crate::core::events::ReliabilitySettings
    ) -> anyhow::Result<()> {
        // Reset flags
        self.replay_stop_flag.store(false, Ordering::Relaxed);
        self.replay_paused.store(false, Ordering::Relaxed);
        
        self.replayer.execute_with_reliability(
            events, 
            self.replay_stop_flag.clone(),
            reliability
        )
    }

    /// Get element info at coordinates for validation
    pub fn validate_element_at(&self, x: i32, y: i32) -> anyhow::Result<bool> {
        Ok(self.locator.inspect_at(x, y)?.is_some())
    }

    /// Check if replay is currently running
    pub fn is_replay_running(&self) -> bool {
        !self.replay_stop_flag.load(Ordering::Relaxed)
    }

    /// Generate workflow from natural language prompt using LLM
    pub fn generate_workflow_from_prompt(
        &self, 
        prompt: String,
        screenshot: Option<Vec<u8>>,
    ) -> anyhow::Result<Vec<InputEvent>> {
        // Initialize LLM if not already done
        let config = LLMConfig::from_env();
        if get_llm().is_none() {
            llm::init_llm(&config);
        }

        let provider = get_llm()
            .ok_or_else(|| anyhow::anyhow!("No LLM provider available"))?;

        // Get element context from current screen
        let element_context = self.get_visible_elements()?;

        // Call the LLM (async, but we'll block on it for Tauri command)
        let rt = tokio::runtime::Runtime::new()?;
        let events = rt.block_on(async {
            provider.generate_workflow(
                &prompt,
                screenshot.as_deref(),
                None, // AX tree would be populated here
                &element_context,
            ).await
        })?;

        Ok(events)
    }

    /// Get visible elements for context
    fn get_visible_elements(&self) -> anyhow::Result<Vec<ElementInfo>> {
        let mut elements = Vec::new();
        
        // Sample elements at regular intervals
        for y in 0..500 {
            for x in 0..500 {
                if let Ok(Some(el)) = self.locator.inspect_at(x, y) {
                    // Avoid duplicates
                    if !elements.iter().any(|e: &ElementInfo| e.name == el.name && e.role == el.role) {
                        elements.push(el);
                    }
                }
            }
        }

        Ok(elements)
    }

    /// Analyze and add semantic tags to recorded events
    pub fn analyze_and_tag_workflow(&self, events: Vec<InputEvent>) -> anyhow::Result<Vec<InputEvent>> {
        let config = LLMConfig::from_env();
        if get_llm().is_none() {
            llm::init_llm(&config);
        }

        let provider = get_llm()
            .ok_or_else(|| anyhow::anyhow!("No AI provider available"))?;

        let element_context = self.get_visible_elements()?;
        
        let rt = tokio::runtime::Runtime::new()?;
        let tagged_events = rt.block_on(async {
            // Use the analyzer for simpler heuristic-based tagging
            // LLM-based tagging would involve sending the full event stream
            let metadata = WorkflowMetadata::default();
            let analysis = self.analyzer.analyze(&events, &metadata);
            
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

    /// Add semantic context to an event
    fn add_semantic_context(
        &self, 
        event: &InputEvent, 
        elements: &[ElementInfo]
    ) -> InputEvent {
        match event {
            InputEvent::MouseClick { x, y, button, element, .. } => {
                let semantic_tag = element.as_ref()
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
                    timestamp: None,
                    retry_count: None,
                    semantic_tag,
                    self_heal: Some(true),
                }
            }
            InputEvent::Key { code, chars, modifiers, action, .. } => {
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
                    timestamp: None,
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
        elements: &[ElementInfo]
    ) -> Option<ElementInfo> {
        elements.iter()
            .filter_map(|el| el.fallback_coords.as_ref())
            .filter_map(|(ex, ey)| {
                let dist = ((x - ex).pow(2) + (y - ey).pow(2)) as f32;
                Some((el, dist))
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
        smart_wait(condition, self.locator.as_ref(), timeout_ms, poll_interval_ms)
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
}

impl Default for GhostEngine {
    fn default() -> Self {
        Self::new()
    }
}
