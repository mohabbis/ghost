//! Ghost Tauri commands - platform-agnostic IPC handlers.

use crate::engine::GhostEngine;
use crate::core::events::InputEvent;
use std::sync::mpsc;
use tauri::{AppHandle, Emitter, State};
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

/// Delete a workflow from disk.
#[tauri::command]
pub fn delete_workflow(name: String, engine: State<GhostEngine>) -> Result<(), String> {
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

/// Check accessibility permissions (platform-agnostic stub).
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

/// Request accessibility permissions (platform-agnostic stub).
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

// ===== AI-Powered Workflow Commands =====

/// Analyze a workflow and return AI-powered insights
#[tauri::command]
pub fn analyze_workflow(name: String, events: Vec<InputEvent>, engine: State<GhostEngine>) -> crate::core::events::WorkflowAnalysis {
    engine.analyze_workflow(&events, &name)
}

/// Generate an optimized version of a workflow
#[tauri::command]
pub fn optimize_workflow(events: Vec<InputEvent>, engine: State<GhostEngine>) -> Result<Vec<InputEvent>, String> {
    use crate::core::ai::WorkflowOptimizer;
    
    let optimizer = WorkflowOptimizer::new();
    optimizer.optimize(&events).map_err(|e| e.to_string())
}

/// Generate a workflow name suggestion
#[tauri::command]
pub fn suggest_workflow_name(events: Vec<InputEvent>, engine: State<GhostEngine>) -> String {
    engine.generate_workflow_name(&events).unwrap_or_else(|_| "Workflow".to_string())
}

/// Save a workflow with full metadata
#[tauri::command]
pub fn save_workflow_with_metadata(
    name: String, 
    events: Vec<InputEvent>, 
    description: String,
    tags: Vec<String>,
    engine: State<GhostEngine>
) -> Result<String, String> {
    use crate::core::events::WorkflowMetadata;
    use std::time::SystemTime;
    
    let workflow = engine.create_workflow_with_details(&name, &events, &description, &tags);
    
    match engine.save_workflow_with_metadata(&workflow) {
        Ok(path) => Ok(path.to_string_lossy().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Load a workflow with full metadata
#[tauri::command]
pub fn load_workflow_with_metadata(name: String, engine: State<GhostEngine>) -> Result<crate::core::events::Workflow, String> {
    engine.load_workflow_with_metadata(&name).map_err(|e| e.to_string())
}

// ===== Phase 3: AI-Assisted Workflow Generation Commands =====

/// Generate workflow from natural language prompt using LLM
#[tauri::command]
pub fn generate_workflow_from_prompt(
    prompt: String,
    screenshot: Option<Vec<u8>>,
    engine: State<GhostEngine>
) -> Result<Vec<InputEvent>, String> {
    engine.generate_workflow_from_prompt(prompt, screenshot)
        .map_err(|e| e.to_string())
}

/// Analyze recorded events and add semantic tags
#[tauri::command]
pub fn analyze_and_tag_workflow(
    events: Vec<InputEvent>,
    engine: State<GhostEngine>
) -> Result<Vec<InputEvent>, String> {
    engine.analyze_and_tag_workflow(events)
        .map_err(|e| e.to_string())
}

/// Save workflow with semantic metadata sidecar
#[tauri::command]
pub fn save_workflow_with_sidecar(
    name: String,
    events: Vec<InputEvent>,
    description: String,
    tags: Vec<String>,
    engine: State<GhostEngine>
) -> Result<String, String> {
    use crate::core::events::WorkflowMetadata;
    use std::fs;
    use std::time::SystemTime;

    let tagged_events = engine.analyze_and_tag_workflow(events.clone())?;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Save main workflow file
    let workflow = engine.create_workflow_with_details(&name, &tagged_events, &description, &tags);
    engine.save_workflow_with_metadata(&workflow)
        .map(|p| p.to_string_lossy().to_string())?;

    // Save sidecar metadata file
    let data_dir = tauri::api::path::data_dir()
        .ok_or_else(|| "Could not determine data directory".to_string())?;
    let meta_path = data_dir.join("ghost").join("workflows").join(format!("{}.meta.json", name));
    
    let meta = serde_json::json!({
        "workflow_name": name,
        "description": description,
        "tags": tags,
        "created_at": now,
        "ai_generated": false,
        "semantic_tags": tagged_events.iter()
            .filter_map(|e| match e {
                InputEvent::MouseClick { semantic_tag, .. } |
                InputEvent::Key { semantic_tag, .. } => semantic_tag.as_ref().map(|t| {
                    serde_json::json!({
                        "action": &t.action,
                        "target": &t.target,
                        "confidence": t.confidence
                    })
                }),
                _ => None
            })
            .collect::<Vec<_>>()
    });

    fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)
        .map_err(|e| e.to_string())?;

    Ok(name)
}

// ===== Reliability Feature Commands =====

/// Execute a workflow with reliability features
#[tauri::command]
pub fn replay_with_reliability(
    events: Vec<InputEvent>, 
    max_attempts: Option<u32>,
    backoff_ms: Option<u64>,
    backoff_multiplier: Option<f32>,
    checkpoints: Option<Vec<crate::core::events::Checkpoint>>,
    engine: State<GhostEngine>
) -> Result<(), String> {
    let reliability = crate::core::events::ReliabilitySettings {
        retry_config: crate::core::events::RetryConfig {
            max_attempts: max_attempts.unwrap_or(3),
            backoff_ms: backoff_ms.unwrap_or(500),
            backoff_multiplier: backoff_multiplier.unwrap_or(2.0),
        },
        checkpoints: checkpoints.unwrap_or_default(),
        ..Default::default()
    };
    
    engine.replay_with_reliability(&events, &reliability)
        .map_err(|e| e.to_string())
}

// ===== Cloud Sync Commands =====

use crate::core::cloud::{CloudConfig, CloudSyncManager, Workspace, AuditLog};
use std::sync::Mutex;

/// Cloud sync state - managed by Tauri
pub struct CloudState {
    pub manager: Mutex<Option<CloudSyncManager>>,
}

impl Default for CloudState {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudState {
    pub fn new() -> Self {
        CloudState {
            manager: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn init_cloud_sync(config: CloudConfig, state: tauri::State<'_, CloudState>) -> Result<bool, String> {
    let manager = CloudSyncManager::new(config);
    *state.manager.lock().unwrap() = Some(manager);
    Ok(true)
}

#[tauri::command]
pub fn cloud_authenticate(token: String, state: tauri::State<'_, CloudState>) -> Result<bool, String> {
    let mut manager_lock = state.manager.lock().unwrap();
    if let Some(manager) = manager_lock.as_mut() {
        manager.authenticate(token).map_err(|e| e.to_string())
    } else {
        Err("Cloud sync not initialized".to_string())
    }
}

#[tauri::command]
pub fn cloud_sync_workflows(
    name: String,
    events: Vec<InputEvent>, 
    description: String,
    state: tauri::State<'_, CloudState>
) -> Result<Vec<String>, String> {
    let manager_lock = state.manager.lock().unwrap();
    if let Some(manager) = manager_lock.as_ref() {
        // Convert events to workflow with proper metadata
        let workflow = crate::core::events::Workflow {
            name,
            events,
            metadata: crate::core::events::WorkflowMetadata {
                name: description.clone(),
                description,
                tags: vec!["synced".to_string()],
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                updated_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                estimated_duration_ms: 0,
                reliability_score: 1.0,
                element_confidence: 1.0,
            },
            reliability: None,
        };
        manager.sync_workflows(&[workflow]).map_err(|e| e.to_string())
    } else {
        Err("Cloud sync not initialized".to_string())
    }
}

#[tauri::command]
pub fn create_workspace(name: String, owner_id: String, state: tauri::State<'_, CloudState>) -> Result<Workspace, String> {
    let mut manager_lock = state.manager.lock().unwrap();
    if let Some(manager) = manager_lock.as_mut() {
        Ok(manager.create_workspace(name, owner_id))
    } else {
        Err("Cloud sync not initialized".to_string())
    }
}

#[tauri::command]
pub fn get_audit_logs(limit: Option<usize>, state: tauri::State<'_, CloudState>) -> Result<Vec<AuditLog>, String> {
    let manager_lock = state.manager.lock().unwrap();
    if let Some(manager) = manager_lock.as_ref() {
        Ok(manager.get_audit_logs(limit).into_iter().cloned().collect())
    } else {
        Err("Cloud sync not initialized".to_string())
    }
}
