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
            commands::inspect_element,
            commands::check_accessibility,
            commands::request_accessibility
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
