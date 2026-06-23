use crate::core::events::InputEvent;
use crate::core::security;
use crate::engine::GhostEngine;
use std::sync::mpsc;
use tauri::{AppHandle, Emitter, Manager, State};

/// Spawns a thread to bridge native events to Tauri IPC.
#[tauri::command]
pub fn start_recording(app: AppHandle, engine: State<GhostEngine>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use crate::platform::macos::MacosBackend;
        if !MacosBackend::check_accessibility() {
            return Err(
                "Accessibility permission is not granted. Open System Settings -> Privacy & Security -> Accessibility, enable Ghost, then restart the app.".into(),
            );
        }
        if !MacosBackend::check_input_monitoring() {
            return Err(
                "Input Monitoring permission is not granted (needed to capture keystrokes). Open System Settings -> Privacy & Security -> Input Monitoring, enable Ghost, then restart the app.".into(),
            );
        }
    }

    let (tx, rx) = mpsc::channel::<InputEvent>();
    engine.start_recording(tx).map_err(|e| e.to_string())?;

    let app_handle = app.clone();
    std::thread::spawn(move || {
        let mut suppress_keyboard = false;
        while let Ok(event) = rx.recv() {
            if crate::core::guard::should_suppress_keyboard_after_click(&event) {
                suppress_keyboard = true;
                let _ = app_handle.emit(
                    "ghost:guard",
                    "Sensitive input detected; keyboard capture paused until you click a non-sensitive field.",
                );
                continue;
            }

            if matches!(event, InputEvent::MouseClick { .. }) {
                suppress_keyboard = false;
            }

            let Some(mut event) =
                crate::core::guard::sanitize_recorded_event(event, suppress_keyboard)
            else {
                let _ = app_handle.emit(
                    "ghost:guard",
                    "Ghost Guard suppressed sensitive keyboard input.",
                );
                continue;
            };

            if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                event.set_timestamp(now.as_millis() as u64);
            }

            let engine = app_handle.state::<GhostEngine>();
            engine.buffer_event(event.clone());

            if let Err(e) = app_handle.emit("ghost:event", event) {
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

/// Run Ghost Guard's local privacy/cybersecurity audit against a workflow.
#[tauri::command]
pub fn ghost_guard_audit(events: Vec<InputEvent>) -> crate::core::guard::GhostGuardReport {
    crate::core::guard::audit_workflow(&events)
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

/// Check if replay is currently running.
#[tauri::command]
pub fn is_replay_running(engine: State<GhostEngine>) -> bool {
    engine.is_replay_running()
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
pub fn inspect_element(
    x: i32,
    y: i32,
    engine: State<GhostEngine>,
) -> Result<Option<crate::core::events::ElementInfo>, String> {
    engine.inspect_element(x, y).map_err(|e| e.to_string())
}

/// Result of inspecting the element under the mouse cursor.
#[derive(serde::Serialize)]
pub struct CursorInspection {
    pub x: i32,
    pub y: i32,
    pub element: Option<crate::core::events::ElementInfo>,
}

/// Inspect the UI element under the current mouse cursor position.
#[tauri::command]
pub fn inspect_element_at_cursor(engine: State<GhostEngine>) -> Result<CursorInspection, String> {
    use enigo::{Enigo, Mouse, Settings};
    let enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let (x, y) = enigo.location().map_err(|e| e.to_string())?;
    let element = engine.inspect_element(x, y).map_err(|e| e.to_string())?;
    Ok(CursorInspection { x, y, element })
}

/// Save a workflow to disk.
#[tauri::command]
pub fn save_workflow(
    name: String,
    events: Vec<InputEvent>,
    engine: State<GhostEngine>,
) -> Result<String, String> {
    security::sanitize_workflow_path(&name).map_err(|e| e.to_string())?;
    match engine.save_workflow(&name, &events) {
        Ok(path) => Ok(path.to_string_lossy().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Load a workflow from disk.
#[tauri::command]
pub fn load_workflow(name: String, engine: State<GhostEngine>) -> Result<Vec<InputEvent>, String> {
    security::sanitize_workflow_path(&name).map_err(|e| e.to_string())?;
    engine.load_workflow(&name).map_err(|e| e.to_string())
}

/// Delete a workflow from disk.
#[tauri::command]
pub fn delete_workflow(name: String, engine: State<GhostEngine>) -> Result<(), String> {
    security::sanitize_workflow_path(&name).map_err(|e| e.to_string())?;
    engine.delete_workflow(&name).map_err(|e| e.to_string())
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

/// Check accessibility permissions.
#[tauri::command]
pub fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        use crate::platform::macos::MacosBackend;
        MacosBackend::check_accessibility()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Request accessibility permissions.
#[tauri::command]
pub fn request_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        use crate::platform::macos::MacosBackend;
        MacosBackend::request_accessibility()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Check Input Monitoring permission.
#[tauri::command]
pub fn check_input_monitoring() -> bool {
    #[cfg(target_os = "macos")]
    {
        use crate::platform::macos::MacosBackend;
        MacosBackend::check_input_monitoring()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Request Input Monitoring permission.
#[tauri::command]
pub fn request_input_monitoring() -> bool {
    #[cfg(target_os = "macos")]
    {
        use crate::platform::macos::MacosBackend;
        MacosBackend::request_input_monitoring()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}
