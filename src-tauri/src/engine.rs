//! Ghost engine: platform-agnostic orchestration layer.
//! Manages recording, element lookup, and replay with cancellation support.

use crate::core::events::InputEvent;
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
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
}

impl Default for GhostEngine {
    fn default() -> Self {
        Self::new()
    }
}
