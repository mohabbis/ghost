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

/// Inspect the UI element at the given screen coordinates.
#[tauri::command]
pub fn inspect_element(x: i32, y: i32, engine: State<GhostEngine>) -> Result<Option<crate::core::events::ElementInfo>, String> {
    engine.inspect_element(x, y).map_err(|e| e.to_string())
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
