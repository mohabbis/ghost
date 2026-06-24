pub mod auth;
mod commands;
pub mod config;
pub mod core;
pub mod engine;
pub mod error;
pub mod organizer;
pub mod performance;
mod platform;
pub mod policy;
pub mod storage;
pub mod telemetry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(engine::GhostEngine::new())
        .manage(commands::CloudState::default())
        .invoke_handler(tauri::generate_handler![
            // Stable core: recording, replay, inspection, workflow storage, and permissions.
            commands::start_recording,
            commands::stop_recording,
            commands::replay_workflow,
            commands::ghost_guard_audit,
            commands::cancel_replay,
            commands::pause_replay,
            commands::resume_replay,
            commands::is_replay_paused,
            commands::is_replay_running,
            commands::set_playback_speed,
            commands::get_playback_speed,
            commands::inspect_element,
            commands::inspect_element_at_cursor,
            commands::save_workflow,
            commands::load_workflow,
            commands::delete_workflow,
            commands::list_workflows,
            commands::get_recorded_events,
            commands::check_accessibility,
            commands::request_accessibility,
            commands::check_input_monitoring,
            commands::request_input_monitoring,
            // Local auth and at-rest workflow protection.
            commands::auth_status,
            commands::auth_setup,
            commands::auth_unlock,
            commands::auth_lock,
            // Configuration, telemetry, and diagnostics.
            commands::get_config,
            commands::update_config,
            commands::get_telemetry_stats,
            commands::export_telemetry,
            commands::get_performance_summary,
            // Experimental surfaces. These remain registered for frontend compatibility,
            // but they are product-boundary candidates for feature flags or a separate
            // command namespace before Ghost is presented as user-ready.
            commands::analyze_workflow,
            commands::optimize_workflow,
            commands::suggest_workflow_name,
            commands::save_workflow_with_metadata,
            commands::load_workflow_with_metadata,
            commands::generate_workflow_from_prompt,
            commands::analyze_and_tag_workflow,
            commands::save_workflow_with_sidecar,
            commands::replay_with_reliability,
            commands::init_cloud_sync,
            commands::cloud_authenticate,
            commands::cloud_sync_workflows,
            commands::create_workspace,
            commands::get_audit_logs,
            commands::get_execution_history,
            commands::get_all_executions,
            commands::get_workflow_analytics,
            commands::replay_with_visual_check,
            commands::capture_baseline_screenshot,
            commands::create_data_source,
            commands::load_variables,
            commands::start_observer,
            commands::stop_observer,
            commands::is_observer_active,
            commands::set_observer_interval,
            commands::observe_events,
            commands::get_proactive_suggestions,
            commands::get_learned_patterns,
            commands::get_app_usage_stats,
            commands::generate_geek_insights,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
