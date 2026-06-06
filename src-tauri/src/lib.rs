mod commands;

#[cfg(target_os = "macos")]
mod macos_ax;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::check_accessibility,
            commands::request_accessibility,
            commands::start_recording,
            commands::stop_recording,
            commands::replay_click
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
