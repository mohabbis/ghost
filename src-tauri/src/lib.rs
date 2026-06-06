mod core;
mod engine;
mod platform;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(engine::GhostEngine::new())
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::replay_workflow,
            commands::cancel_replay,
            commands::pause_replay,
            commands::resume_replay,
            commands::is_replay_paused,
            commands::set_playback_speed,
            commands::get_playback_speed,
            commands::inspect_element,
            commands::save_workflow,
            commands::load_workflow,
            commands::list_workflows,
            commands::get_recorded_events,
            commands::check_accessibility,
            commands::request_accessibility
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
