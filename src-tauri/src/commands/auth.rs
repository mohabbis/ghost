use crate::engine::GhostEngine;
use tauri::State;

/// Combined auth state for the frontend lock screen / onboarding.
#[derive(serde::Serialize)]
pub struct AuthStatus {
    pub configured: bool,
    pub unlocked: bool,
}

/// Whether a local password exists and whether the app is currently unlocked.
#[tauri::command]
pub fn auth_status(engine: State<GhostEngine>) -> AuthStatus {
    let auth = engine.auth();
    AuthStatus {
        configured: auth.is_configured(),
        unlocked: auth.is_unlocked(),
    }
}

/// Create the local password. Leaves the app unlocked.
#[tauri::command]
pub fn auth_setup(password: String, engine: State<GhostEngine>) -> Result<(), String> {
    engine.auth().setup(&password).map_err(|e| e.to_string())
}

/// Try to unlock with the given password. Returns false on a wrong password.
#[tauri::command]
pub fn auth_unlock(password: String, engine: State<GhostEngine>) -> Result<bool, String> {
    engine.auth().unlock(&password).map_err(|e| e.to_string())
}

/// Lock the app: drops the in-memory key until the next unlock.
#[tauri::command]
pub fn auth_lock(engine: State<GhostEngine>) {
    engine.auth().lock();
}
