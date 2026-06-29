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

/// Get the collected usage telemetry statistics.
#[tauri::command]
pub fn get_telemetry_stats(engine: State<GhostEngine>) -> crate::telemetry::UsageStats {
    engine.get_telemetry_stats()
}

/// Export all collected telemetry as a JSON string.
#[tauri::command]
pub fn export_telemetry(engine: State<GhostEngine>) -> Result<String, String> {
    engine.export_telemetry().map_err(|e| e.to_string())
}

/// Get a summary of recorded performance metrics.
#[tauri::command]
pub fn get_performance_summary(
    engine: State<GhostEngine>,
) -> crate::performance::PerformanceSummary {
    engine.get_performance_summary()
}

/// Report whether this build was compiled with the experimental command
/// surface enabled. Always registered so the frontend can feature-detect and
/// hide experimental controls rather than calling commands that do not exist.
#[tauri::command]
pub fn is_experimental_enabled() -> bool {
    cfg!(feature = "experimental")
}
