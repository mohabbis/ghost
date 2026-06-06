//! Ghost Tauri commands - platform-agnostic IPC handlers.

use crate::engine::GhostEngine;
use crate::core::events::InputEvent;
use std::sync::mpsc;
use tauri::{AppHandle, Emitter, State};

/// Start recording input events.
/// Spawns a thread to bridge native events → Tauri IPC.
#[tauri::command]
pub fn start_recording(app: AppHandle, engine: State<GhostEngine>) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<InputEvent>();

    // Start the native recorder
    engine.start_recording(tx).map_err(|e| e.to_string())?;

    // Spawn bridge thread: consume from mpsc and emit to frontend
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            // Buffer event in engine
            engine.buffer_event(event.clone());
            
            // Emit serialized event to frontend
            if let Err(e) = app.emit("ghost:event", event) {
                eprintln!("Failed to emit event: {}", e);
                break;
            }
        }
    });

    Ok(())
}

/// Stop the active recording session.
#[tauri::command]
pub fn stop_recording(engine: State<GhostEngine>) {
    engine.stop_recording();
}

/// Replay a workflow of recorded events.
#[tauri::command]
pub fn replay_workflow(events: Vec<InputEvent>, engine: State<GhostEngine>) -> Result<(), String> {
    engine.replay(&events).map_err(|e| e.to_string())
}

/// Cancel an ongoing replay immediately.
#[tauri::command]
pub fn cancel_replay(engine: State<GhostEngine>) {
    engine.cancel_replay();
}

/// Pause an ongoing replay.
#[tauri::command]
pub fn pause_replay(engine: State<GhostEngine>) {
    engine.pause_replay();
}

/// Resume a paused replay.
#[tauri::command]
pub fn resume_replay(engine: State<GhostEngine>) {
    engine.resume_replay();
}

/// Check if replay is currently paused.
#[tauri::command]
pub fn is_replay_paused(engine: State<GhostEngine>) -> bool {
    engine.is_replay_paused()
}

/// Set the playback speed factor.
#[tauri::command]
pub fn set_playback_speed(factor: f32, engine: State<GhostEngine>) -> Result<(), String> {
    engine.set_playback_speed(factor);
    Ok(())
}

/// Get the current playback speed factor.
#[tauri::command]
pub fn get_playback_speed(engine: State<GhostEngine>) -> f32 {
    engine.get_playback_speed()
}

/// Inspect the UI element at the given screen coordinates.
#[tauri::command]
pub fn inspect_element(x: i32, y: i32, engine: State<GhostEngine>) -> Result<Option<crate::core::events::ElementInfo>, String> {
    engine.inspect_element(x, y).map_err(|e| e.to_string())
}

/// Save a workflow to disk.
#[tauri::command]
pub fn save_workflow(name: String, events: Vec<InputEvent>, engine: State<GhostEngine>) -> Result<String, String> {
    match engine.save_workflow(&name, &events) {
        Ok(path) => Ok(path.to_string_lossy().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Load a workflow from disk.
#[tauri::command]
pub fn load_workflow(name: String, engine: State<GhostEngine>) -> Result<Vec<InputEvent>, String> {
    engine.load_workflow(&name).map_err(|e| e.to_string())
}

/// List all saved workflows.
#[tauri::command]
pub fn list_workflows() -> Result<Vec<String>, String> {
    GhostEngine::list_workflows().map_err(|e| e.to_string())
}

/// Get all recorded events from the current session.
#[tauri::command]
pub fn get_recorded_events(engine: State<GhostEngine>) -> Vec<InputEvent> {
    engine.get_recorded_events()
}

/// Check accessibility permissions (platform-agnostic stub).
#[tauri::command]
pub fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        // TODO: Call platform-specific permission check
        true
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Request accessibility permissions (platform-agnostic stub).
#[tauri::command]
pub fn request_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        // TODO: Call platform-specific permission request
        true
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}
