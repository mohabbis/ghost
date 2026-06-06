//! Ghost engine: platform-agnostic orchestration layer.
//! Manages recording, element lookup, and replay with cancellation support.

use crate::core::events::InputEvent;
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

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
        }
    }

    /// Start recording input events. Events will be sent through the provided channel.
    pub fn start_recording(&self, tx: mpsc::Sender<InputEvent>) -> anyhow::Result<()> {
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

    /// Replay a sequence of recorded events.
    pub fn replay(&self, events: &[InputEvent]) -> anyhow::Result<()> {
        // Reset the stop flag before starting
        self.replay_stop_flag.store(false, Ordering::Relaxed);
        self.replayer.execute(events, self.replay_stop_flag.clone())
    }

    /// Cancel an ongoing replay immediately.
    pub fn cancel_replay(&self) {
        self.replay_stop_flag.store(true, Ordering::Relaxed);
    }

    /// Get the element info at the given screen coordinates.
    pub fn inspect_element(&self, x: i32, y: i32) -> anyhow::Result<Option<crate::core::events::ElementInfo>> {
        self.locator.inspect_at(x, y)
    }

    /// Get a clone of the replay stop flag for external monitoring.
    pub fn get_stop_flag(&self) -> Arc<AtomicBool> {
        self.replay_stop_flag.clone()
    }
}

impl Default for GhostEngine {
    fn default() -> Self {
        Self::new()
    }
}
