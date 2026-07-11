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

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Manager;

    /// A mock Tauri app managing a fresh `GhostEngine`, so these tests
    /// exercise the real `State<GhostEngine>` extraction each command runs
    /// under rather than calling the inner engine methods directly.
    fn managed_test_app() -> tauri::App<tauri::test::MockRuntime> {
        let app = tauri::test::mock_app();
        app.manage(GhostEngine::new());
        app
    }

    #[test]
    fn is_experimental_enabled_matches_the_compiled_feature() {
        assert_eq!(is_experimental_enabled(), cfg!(feature = "experimental"));
    }

    #[test]
    fn get_config_returns_the_engines_persisted_config() {
        let app = managed_test_app();
        let engine = app.state::<GhostEngine>();
        assert_eq!(
            get_config(engine.clone()).replay.default_speed,
            engine.get_config().replay.default_speed
        );
    }

    #[test]
    fn update_config_round_trips_through_get_config() {
        let app = managed_test_app();
        let engine = app.state::<GhostEngine>();
        let mut config = get_config(engine.clone());
        config.replay.default_speed = 2.5;

        update_config(config.clone(), engine.clone()).expect("valid config should update");

        assert_eq!(get_config(engine).replay.default_speed, 2.5);
    }

    #[test]
    fn update_config_rejects_invalid_config_and_leaves_state_unchanged() {
        let app = managed_test_app();
        let engine = app.state::<GhostEngine>();
        let before = get_config(engine.clone());

        let mut invalid = before.clone();
        invalid.audit.retention_keep_last = Some(0);

        let err = update_config(invalid, engine.clone())
            .expect_err("retention_keep_last = 0 must be rejected");
        assert!(!err.is_empty());
        assert_eq!(
            get_config(engine).audit.retention_keep_last,
            before.audit.retention_keep_last,
            "a rejected config must not be applied"
        );
    }

    #[test]
    fn telemetry_and_performance_summaries_are_reachable_through_the_command_layer() {
        let app = managed_test_app();
        let engine = app.state::<GhostEngine>();

        // Fresh engine: telemetry off by default, nothing collected yet.
        let stats = get_telemetry_stats(engine.clone());
        assert_eq!(stats.workflows_recorded, 0);
        assert!(stats.feature_usage.is_empty());

        // export_telemetry must succeed (valid JSON) even with nothing collected.
        let exported = export_telemetry(engine.clone()).expect("export should not fail");
        assert!(serde_json::from_str::<serde_json::Value>(&exported).is_ok());

        let perf = get_performance_summary(engine);
        assert!(perf.operations.is_empty());
    }
}
