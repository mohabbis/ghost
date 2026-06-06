mod core;
mod engine;
mod platform;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(engine::GhostEngine::new())
        .manage(commands::CloudState::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::replay_workflow,
            commands::cancel_replay,
            commands::pause_replay,
            commands::resume_replay,
            commands::is_replay_paused,
            commands::is_replay_running,
            commands::set_playback_speed,
            commands::get_playback_speed,
            commands::inspect_element,
            commands::save_workflow,
            commands::load_workflow,
            commands::delete_workflow,
            commands::list_workflows,
            commands::get_recorded_events,
            commands::check_accessibility,
            commands::request_accessibility,
            // AI-powered commands (Phase 1 & 3)
            commands::analyze_workflow,
            commands::optimize_workflow,
            commands::suggest_workflow_name,
            commands::save_workflow_with_metadata,
            commands::load_workflow_with_metadata,
            // Phase 3: AI-Assisted Workflow Generation
            commands::generate_workflow_from_prompt,
            commands::analyze_and_tag_workflow,
            commands::save_workflow_with_sidecar,
            // Reliability commands
            commands::replay_with_reliability,
            // Cloud sync commands
            commands::init_cloud_sync,
            commands::cloud_authenticate,
            commands::cloud_sync_workflows,
            commands::create_workspace,
            commands::get_audit_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
