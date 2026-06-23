//! Configuration, telemetry, performance, execution history, and analytics commands.

use crate::engine::GhostEngine;
use tauri::State;

/// Get the current persisted configuration.
#[tauri::command]
pub fn get_config(engine: State<GhostEngine>) -> crate::config::GhostConfig {
    engine.get_config()
}

/// Validate, persist, and apply a new configuration.
#[tauri::command]
pub fn update_config(
    config: crate::config::GhostConfig,
    engine: State<GhostEngine>,
) -> Result<(), String> {
    engine.update_config(config).map_err(|e| e.to_string())
}

/// Get the collected usage telemetry statistics (empty unless opted in).
#[tauri::command]
pub fn get_telemetry_stats(engine: State<GhostEngine>) -> crate::telemetry::UsageStats {
    engine.get_telemetry_stats()
}

/// Export all collected telemetry as a JSON string.
#[tauri::command]
pub fn export_telemetry(engine: State<GhostEngine>) -> Result<String, String> {
    engine.export_telemetry().map_err(|e| e.to_string())
}

/// Get a summary of recorded performance metrics (empty unless profiling is on).
#[tauri::command]
pub fn get_performance_summary(
    engine: State<GhostEngine>,
) -> crate::performance::PerformanceSummary {
    engine.get_performance_summary()
}

/// Get execution history for a workflow.
#[tauri::command]
pub fn get_execution_history(
    workflow_name: String,
    engine: State<GhostEngine>,
) -> Result<Vec<crate::core::execution::ExecutionRecord>, String> {
    let tracker = engine.get_execution_tracker();
    match tracker.as_ref().and_then(|guard| guard.as_ref()) {
        Some(history) => history
            .get_history(&workflow_name)
            .map_err(|e| e.to_string()),
        None => Ok(Vec::new()),
    }
}

/// Get all execution records.
#[tauri::command]
pub fn get_all_executions(
    limit: Option<usize>,
    engine: State<GhostEngine>,
) -> Result<Vec<crate::core::execution::ExecutionRecord>, String> {
    let tracker = engine.get_execution_tracker();
    match tracker.as_ref().and_then(|guard| guard.as_ref()) {
        Some(history) => history.get_all_records(limit).map_err(|e| e.to_string()),
        None => Ok(Vec::new()),
    }
}

/// Get workflow analytics summary.
#[tauri::command]
pub fn get_workflow_analytics(
    workflow_name: String,
    engine: State<GhostEngine>,
) -> Result<serde_json::Value, String> {
    let tracker = engine.get_execution_tracker();
    if let Some(history) = tracker.as_ref().and_then(|guard| guard.as_ref()) {
        let success_rate = history.get_success_rate(&workflow_name).unwrap_or(1.0);
        let avg_duration = history.get_avg_duration(&workflow_name).unwrap_or(0);
        let hotspots = history
            .get_failure_hotspots(&workflow_name)
            .unwrap_or_default();

        Ok(serde_json::json!({
            "workflow_name": workflow_name,
            "success_rate": success_rate,
            "average_duration_ms": avg_duration,
            "failure_hotspots": hotspots,
            "total_executions": history.get_history(&workflow_name).map(|r| r.len()).unwrap_or(0)
        }))
    } else {
        Err("Execution tracker not initialized".to_string())
    }
}
