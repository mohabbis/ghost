use tauri::AppHandle;
use crate::macos_ax;

#[tauri::command]
pub fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    return macos_ax::check_permissions();
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[tauri::command]
pub fn request_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    return macos_ax::request_permissions();
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[tauri::command]
pub fn start_recording(handle: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos_ax::start_event_tap(handle);
    #[cfg(not(target_os = "macos"))]
    return Err("Mac only".into());
}

#[tauri::command]
pub fn stop_recording() {
    #[cfg(target_os = "macos")]
    macos_ax::stop_event_tap();
}

#[tauri::command]
pub fn replay_click(x: f64, y: f64) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos_ax::click_at(x, y);
    #[cfg(not(target_os = "macos"))]
    return Err("Mac only".into());
}
