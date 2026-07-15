//! Core traits for platform-agnostic input handling.
//! All traits are Send + Sync for thread-safe cross-platform operation.

use crate::core::events::{ElementInfo, InputEvent, ReliabilitySettings};
use crate::core::replay_support::ReplayProgress;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Trait for recording input events from the native system.
/// Implementations must be thread-safe (Send + Sync).
pub trait InputRecorder: Send + Sync {
    /// Start recording input events, sending them through the provided channel.
    /// Returns an error if recording cannot be started.
    fn start(&self, tx: std::sync::mpsc::Sender<InputEvent>) -> anyhow::Result<()>;

    /// Stop the active recording session.
    fn stop(&self);
}

/// Trait for locating UI elements via accessibility APIs.
pub trait ElementLocator: Send + Sync {
    /// Inspect the UI element at the given screen coordinates.
    /// Returns element metadata if found.
    fn inspect_at(&self, x: i32, y: i32) -> anyhow::Result<Option<ElementInfo>>;
}

/// Trait for replaying recorded input events.
pub trait ReplayEngine: Send + Sync {
    /// Execute a sequence of input events.
    /// `stop_flag` cancels replay from another thread; `pause_flag` suspends
    /// it (the loop blocks between events until resumed or cancelled);
    /// `speed` scales recorded delays/pacing (1.0 = real time, clamped ≥ 0.1);
    /// `progress` is advanced to each event's index just before it executes so
    /// the engine/UI can render per-step status while the loop runs.
    fn execute(
        &self,
        events: &[InputEvent],
        stop_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        speed: f32,
        progress: Arc<ReplayProgress>,
    ) -> anyhow::Result<()>;

    /// Execute a sequence of input events with reliability features.
    /// Supports retry logic, checkpoints, and validation.
    fn execute_with_reliability(
        &self,
        events: &[InputEvent],
        stop_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        speed: f32,
        progress: Arc<ReplayProgress>,
        reliability: &ReliabilitySettings,
    ) -> anyhow::Result<()>;
}
