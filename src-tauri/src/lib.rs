mod commands;
#[cfg(target_os = "macos")]
mod macos_ax;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickEvent {
    pub x: f64,
    pub y: f64,
    pub title: String,
    pub role: String,
    pub timestamp: u64,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::check_accessibility,
            commands::request_accessibility,
            commands::start_recording,
            commands::stop_recording,
            commands::replay_click,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Ghost");
}
