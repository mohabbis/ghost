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
pub fn analyze_workflow(name: String, events: Vec<InputEvent>, engine: State<GhostEngine>) -> crate::core::ai::WorkflowAnalysis {
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
    let data_dir = dirs::data_dir()
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

// ===== Phase 4A: Visual Regression Replay Commands =====

/// Execute a workflow with visual regression checks
#[tauri::command]
pub fn replay_with_visual_check(
    events: Vec<InputEvent>,
    visual_checks: Vec<crate::core::events::VisualCheckPoint>,
    engine: State<GhostEngine>,
) -> Result<bool, String> {
    engine
        .replay_with_visual_check(&events, &visual_checks)
        .map_err(|e| e.to_string())
}

/// Capture and save a baseline screenshot for visual regression testing
#[tauri::command]
pub fn capture_baseline_screenshot(
    name: String,
    region: Option<(i32, i32, i32, i32)>, // x, y, width, height
    engine: State<GhostEngine>,
) -> Result<String, String> {
    engine
        .capture_baseline(&name, region)
        .map_err(|e| e.to_string())
}

// ===== Phase 4C: Data-Driven Testing Commands =====

/// Create a data source for variable-driven workflows
#[tauri::command]
pub fn create_data_source(
    name: String,
    source_type: String, // "csv", "json", "environment"
    path: Option<String>,
    engine: State<GhostEngine>,
) -> Result<String, String> {
    engine
        .create_data_source(&name, &source_type, path.as_deref())
        .map_err(|e| e.to_string())
}

/// Load variables from a data source
#[tauri::command]
pub fn load_variables(
    data_source_name: String,
    engine: State<GhostEngine>,
) -> Result<std::collections::HashMap<String, String>, String> {
    engine
        .load_variables(&data_source_name)
        .map_err(|e| e.to_string())
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
    name: Option<String>,
    events: Vec<InputEvent>,
    description: Option<String>,
    state: tauri::State<'_, CloudState>
) -> Result<Vec<String>, String> {
    let manager_lock = state.manager.lock().unwrap();
    if let Some(manager) = manager_lock.as_ref() {
        let name = name.unwrap_or_else(|| "Unnamed Workflow".to_string());
        let description = description.unwrap_or_default();
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

// ===== Phase 5: Execution & Analytics Commands =====

/// Get execution history for a workflow
#[tauri::command]
pub fn get_execution_history(
    workflow_name: String,
    engine: State<GhostEngine>
) -> Result<Vec<crate::core::execution::ExecutionRecord>, String> {
    let history_lock = engine.execution_tracker.lock().unwrap();
    if let Some(ref history) = *history_lock {
        history.get_history(&workflow_name).map_err(|e| e.to_string())
    } else {
        Ok(Vec::new())
    }
}

/// Get all execution records (limited)
#[tauri::command]
pub fn get_all_executions(
    limit: Option<usize>,
    engine: State<GhostEngine>
) -> Result<Vec<crate::core::execution::ExecutionRecord>, String> {
    let history_lock = engine.execution_tracker.lock().unwrap();
    if let Some(ref history) = *history_lock {
        history.get_all_records(limit).map_err(|e| e.to_string())
    } else {
        Ok(Vec::new())
    }
}

/// Get workflow analytics summary
#[tauri::command]
pub fn get_workflow_analytics(
    workflow_name: String,
    engine: State<GhostEngine>
) -> Result<serde_json::Value, String> {
    let history_lock = engine.execution_tracker.lock().unwrap();
    if let Some(ref history) = *history_lock {
        let success_rate = history.get_success_rate(&workflow_name).unwrap_or(1.0);
        let avg_duration = history.get_avg_duration(&workflow_name).unwrap_or(0);
        let hotspots = history.get_failure_hotspots(&workflow_name).unwrap_or_default();
        
        Ok(serde_json::json!({
            "workflow_name": workflow_name,
            "success_rate": success_rate,
            "average_duration_ms": avg_duration,
            "failure_hotspots": hotspots,
            "total_executions": history.get_history(&workflow_name).map(|r| r.len()).unwrap_or(0)
        })
    } else {
        Err("Execution tracker not initialized".to_string())
    }
}

// ===== Phase 4: Smart Observer Mode Commands =====

/// Start the Smart Observer - watch and learn user patterns
#[tauri::command]
pub fn start_observer(engine: State<GhostEngine>) -> Result<bool, String> {
    engine.start_observer();
    Ok(true)
}

/// Stop the Smart Observer
#[tauri::command]
pub fn stop_observer(engine: State<GhostEngine>) -> Result<bool, String> {
    engine.stop_observer();
    Ok(true)
}

/// Check if observer is active
#[tauri::command]
pub fn is_observer_active(engine: State<GhostEngine>) -> bool {
    engine.is_observer_active()
}

/// Set observer interval in milliseconds
#[tauri::command]
pub fn set_observer_interval(interval_ms: u64, engine: State<GhostEngine>) -> Result<(), String> {
    engine.set_observer_interval(interval_ms);
    Ok(())
}

/// Record events as observed patterns
#[tauri::command]
pub fn observe_events(
    events: Vec<InputEvent>,
    app_name: String,
    engine: State<GhostEngine>
) -> Result<u32, String> {
    engine.observe_events(&events, &app_name);
    let patterns = engine.get_learned_patterns(Some(&app_name));
    Ok(patterns.len() as u32)
}

/// Get proactive automation suggestions
#[tauri::command]
pub fn get_proactive_suggestions(engine: State<GhostEngine>) -> Vec<crate::core::knowledge::ProactiveSuggestion> {
    engine.get_proactive_suggestions()
}

/// Get learned patterns (optionally filtered by app)
#[tauri::command]
pub fn get_learned_patterns(
    app_name: Option<String>,
    engine: State<GhostEngine>
) -> Vec<crate::core::knowledge::LearnedPattern> {
    engine.get_learned_patterns(app_name.as_deref())
}

/// Get app usage statistics
#[tauri::command]
pub fn get_app_usage_stats(engine: State<GhostEngine>) -> Vec<crate::core::knowledge::AppUsageStats> {
    engine.get_app_usage_stats()
}

/// Generate geek mode insights for events
#[tauri::command]
pub fn generate_geek_insights(
    events: Vec<InputEvent>,
    app_name: String,
    engine: State<GhostEngine>
) -> crate::core::knowledge::GeekDetails {
    engine.generate_geek_insights(&events, &app_name)
}
