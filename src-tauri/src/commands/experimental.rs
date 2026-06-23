//! Experimental command surfaces kept registered for frontend compatibility.

use crate::{
    core::{
        cloud::{AuditLog, CloudConfig, CloudSyncManager, Workspace},
        events::InputEvent,
        security,
    },
    engine::GhostEngine,
};
use std::sync::Mutex;
use tauri::State;

/// Analyze a workflow and return AI-powered insights.
#[tauri::command]
pub fn analyze_workflow(
    name: String,
    events: Vec<InputEvent>,
    engine: State<GhostEngine>,
) -> crate::core::ai::WorkflowAnalysis {
    engine.track_feature("analyze_workflow");
    engine.analyze_workflow(&events, &name)
}

/// Generate an optimized version of a workflow.
#[tauri::command]
pub fn optimize_workflow(
    events: Vec<InputEvent>,
    engine: State<GhostEngine>,
) -> Result<Vec<InputEvent>, String> {
    use crate::core::ai::WorkflowOptimizer;

    engine.track_feature("optimize_workflow");
    let optimizer = WorkflowOptimizer::new();
    optimizer.optimize(&events).map_err(|e| e.to_string())
}

/// Generate a workflow name suggestion.
#[tauri::command]
pub fn suggest_workflow_name(events: Vec<InputEvent>, engine: State<GhostEngine>) -> String {
    engine
        .generate_workflow_name(&events)
        .unwrap_or_else(|_| "Workflow".to_string())
}

/// Save a workflow with full metadata.
#[tauri::command]
pub fn save_workflow_with_metadata(
    name: String,
    events: Vec<InputEvent>,
    description: String,
    tags: Vec<String>,
    engine: State<GhostEngine>,
) -> Result<String, String> {
    security::sanitize_workflow_path(&name).map_err(|e| e.to_string())?;
    let workflow = engine.create_workflow_with_details(&name, &events, &description, &tags);

    match engine.save_workflow_with_metadata(&workflow) {
        Ok(path) => Ok(path.to_string_lossy().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Load a workflow with full metadata.
#[tauri::command]
pub fn load_workflow_with_metadata(
    name: String,
    engine: State<GhostEngine>,
) -> Result<crate::core::events::Workflow, String> {
    security::sanitize_workflow_path(&name).map_err(|e| e.to_string())?;
    engine
        .load_workflow_with_metadata(&name)
        .map_err(|e| e.to_string())
}

/// Generate workflow from natural language prompt using LLM.
#[tauri::command]
pub fn generate_workflow_from_prompt(
    prompt: String,
    screenshot: Option<Vec<u8>>,
    engine: State<GhostEngine>,
) -> Result<Vec<InputEvent>, String> {
    security::validate_prompt(&prompt).map_err(|e| e.to_string())?;
    engine
        .generate_workflow_from_prompt(prompt, screenshot)
        .map_err(|e| e.to_string())
}

/// Analyze recorded events and add semantic tags.
#[tauri::command]
pub fn analyze_and_tag_workflow(
    events: Vec<InputEvent>,
    engine: State<GhostEngine>,
) -> Result<Vec<InputEvent>, String> {
    engine
        .analyze_and_tag_workflow(events)
        .map_err(|e| e.to_string())
}

/// Save workflow with semantic metadata sidecar.
#[tauri::command]
pub fn save_workflow_with_sidecar(
    name: String,
    events: Vec<InputEvent>,
    description: String,
    tags: Vec<String>,
    engine: State<GhostEngine>,
) -> Result<String, String> {
    use std::time::SystemTime;

    security::sanitize_workflow_path(&name).map_err(|e| e.to_string())?;

    let tagged_events = engine
        .analyze_and_tag_workflow(events.clone())
        .map_err(|e| e.to_string())?;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Save main workflow file.
    let workflow = engine.create_workflow_with_details(&name, &tagged_events, &description, &tags);
    engine
        .save_workflow_with_metadata(&workflow)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())?;

    // Save sidecar metadata file.
    let data_dir =
        dirs::data_dir().ok_or_else(|| "Could not determine data directory".to_string())?;
    let meta_path = data_dir
        .join("ghost")
        .join("workflows")
        .join(format!("{}.meta.json", name));

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

    security::atomic_write(
        &meta_path,
        serde_json::to_string_pretty(&meta)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;

    Ok(name)
}

/// Execute a workflow with visual regression checks.
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

/// Capture and save a baseline screenshot for visual regression testing.
#[tauri::command]
pub fn capture_baseline_screenshot(
    name: String,
    region: Option<(i32, i32, i32, i32)>,
    engine: State<GhostEngine>,
) -> Result<String, String> {
    engine
        .capture_baseline(&name, region)
        .map_err(|e| e.to_string())
}

/// Create a data source for variable-driven workflows.
#[tauri::command]
pub fn create_data_source(
    name: String,
    source_type: String,
    path: Option<String>,
    engine: State<GhostEngine>,
) -> Result<String, String> {
    engine
        .create_data_source(&name, &source_type, path.as_deref())
        .map_err(|e| e.to_string())
}

/// Load variables from a data source.
#[tauri::command]
pub fn load_variables(
    data_source_name: String,
    engine: State<GhostEngine>,
) -> Result<std::collections::HashMap<String, String>, String> {
    engine
        .load_variables(&data_source_name)
        .map_err(|e| e.to_string())
}

/// Execute a workflow with reliability features.
#[tauri::command]
pub fn replay_with_reliability(
    events: Vec<InputEvent>,
    max_attempts: Option<u32>,
    backoff_ms: Option<u64>,
    backoff_multiplier: Option<f32>,
    checkpoints: Option<Vec<crate::core::events::Checkpoint>>,
    engine: State<GhostEngine>,
) -> Result<(), String> {
    // Per-call args override the persisted config defaults.
    let defaults = engine.default_retry_config();
    let reliability = crate::core::events::ReliabilitySettings {
        retry_config: crate::core::events::RetryConfig {
            max_attempts: max_attempts.unwrap_or(defaults.max_attempts),
            backoff_ms: backoff_ms.unwrap_or(defaults.backoff_ms),
            backoff_multiplier: backoff_multiplier.unwrap_or(defaults.backoff_multiplier),
        },
        checkpoints: checkpoints.unwrap_or_default(),
        ..Default::default()
    };

    engine
        .replay_with_reliability(&events, &reliability)
        .map_err(|e| e.to_string())
}

/// Cloud sync state managed by Tauri.
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
pub fn init_cloud_sync(
    config: CloudConfig,
    state: tauri::State<'_, CloudState>,
) -> Result<bool, String> {
    let manager = CloudSyncManager::new(config);
    *state
        .manager
        .lock()
        .map_err(|_| "Cloud sync state lock poisoned".to_string())? = Some(manager);
    Ok(true)
}

#[tauri::command]
pub fn cloud_authenticate(
    token: String,
    state: tauri::State<'_, CloudState>,
) -> Result<bool, String> {
    let mut manager_lock = state
        .manager
        .lock()
        .map_err(|_| "Cloud sync state lock poisoned".to_string())?;
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
    state: tauri::State<'_, CloudState>,
) -> Result<Vec<String>, String> {
    let manager_lock = state
        .manager
        .lock()
        .map_err(|_| "Cloud sync state lock poisoned".to_string())?;
    if let Some(manager) = manager_lock.as_ref() {
        let name = name.unwrap_or_else(|| "Unnamed Workflow".to_string());
        let description = description.unwrap_or_default();
        // Convert events to workflow with proper metadata.
        let workflow = crate::core::events::Workflow {
            schema_version: crate::core::events::current_workflow_schema_version(),
            app_version: crate::core::events::current_app_version(),
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
        manager
            .sync_workflows(&[workflow])
            .map_err(|e| e.to_string())
    } else {
        Err("Cloud sync not initialized".to_string())
    }
}

#[tauri::command]
pub fn create_workspace(
    name: String,
    owner_id: String,
    state: tauri::State<'_, CloudState>,
) -> Result<Workspace, String> {
    let mut manager_lock = state
        .manager
        .lock()
        .map_err(|_| "Cloud sync state lock poisoned".to_string())?;
    if let Some(manager) = manager_lock.as_mut() {
        Ok(manager.create_workspace(name, owner_id))
    } else {
        Err("Cloud sync not initialized".to_string())
    }
}

#[tauri::command]
pub fn get_audit_logs(
    limit: Option<usize>,
    state: tauri::State<'_, CloudState>,
) -> Result<Vec<AuditLog>, String> {
    let manager_lock = state
        .manager
        .lock()
        .map_err(|_| "Cloud sync state lock poisoned".to_string())?;
    if let Some(manager) = manager_lock.as_ref() {
        Ok(manager.get_audit_logs(limit).into_iter().cloned().collect())
    } else {
        Err("Cloud sync not initialized".to_string())
    }
}

/// Start the Smart Observer - watch and learn user patterns.
#[tauri::command]
pub fn start_observer(engine: State<GhostEngine>) -> Result<bool, String> {
    engine.start_observer();
    Ok(true)
}

/// Stop the Smart Observer.
#[tauri::command]
pub fn stop_observer(engine: State<GhostEngine>) -> Result<bool, String> {
    engine.stop_observer();
    Ok(true)
}

/// Check if observer is active.
#[tauri::command]
pub fn is_observer_active(engine: State<GhostEngine>) -> bool {
    engine.is_observer_active()
}

/// Set observer interval in milliseconds.
#[tauri::command]
pub fn set_observer_interval(interval_ms: u64, engine: State<GhostEngine>) -> Result<(), String> {
    engine.set_observer_interval(interval_ms);
    Ok(())
}

/// Record events as observed patterns.
#[tauri::command]
pub fn observe_events(
    events: Vec<InputEvent>,
    app_name: String,
    engine: State<GhostEngine>,
) -> Result<u32, String> {
    engine.observe_events(&events, &app_name);
    let patterns = engine.get_learned_patterns(Some(&app_name));
    Ok(patterns.len() as u32)
}

/// Get proactive automation suggestions.
#[tauri::command]
pub fn get_proactive_suggestions(
    engine: State<GhostEngine>,
) -> Vec<crate::core::knowledge::ProactiveSuggestion> {
    engine.get_proactive_suggestions()
}

/// Get learned patterns (optionally filtered by app).
#[tauri::command]
pub fn get_learned_patterns(
    app_name: Option<String>,
    engine: State<GhostEngine>,
) -> Vec<crate::core::knowledge::LearnedPattern> {
    engine.get_learned_patterns(app_name.as_deref())
}

/// Get app usage statistics.
#[tauri::command]
pub fn get_app_usage_stats(
    engine: State<GhostEngine>,
) -> Vec<crate::core::knowledge::AppUsageStats> {
    engine.get_app_usage_stats()
}

/// Generate geek mode insights for events.
#[tauri::command]
pub fn generate_geek_insights(
    events: Vec<InputEvent>,
    app_name: String,
    engine: State<GhostEngine>,
) -> crate::core::knowledge::GeekDetails {
    engine.generate_geek_insights(&events, &app_name)
}
