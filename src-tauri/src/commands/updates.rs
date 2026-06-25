//! Auto-update commands.
//!
//! These keep auto-update inside Ghost's trust model: AI/automation may
//! *suggest*, deterministic code *applies only what the user approved*. The UI
//! calls [`check_for_update`] (a read-only network check that mutates nothing),
//! shows the result, and only calls [`install_update`] after the user clicks
//! "update now". Updates are verified against the public key embedded from
//! `tauri.conf.json` before they are ever applied — an unsigned or mis-signed
//! payload is rejected by the updater plugin.
//!
//! Risk surface (per `docs/command-registry.md`): network (downloads the update)
//! and app/process state (installs + relaunches). Both are stable, but
//! `install_update` is user-gated by design.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

/// What the UI needs to describe an available update to the user.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    /// The version offered by the update endpoint.
    pub version: String,
    /// The version currently running.
    pub current_version: String,
    /// Release notes, if the endpoint provided them.
    pub notes: Option<String>,
    /// Publish date, if available.
    pub date: Option<String>,
}

/// Check the configured endpoint for a newer signed release. **Read-only**: it
/// downloads nothing and changes nothing, so it is safe to call on launch.
/// Returns `None` when the app is already current.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await.map_err(|e| e.to_string())? {
        Some(update) => Ok(Some(UpdateInfo {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            notes: update.body.clone(),
            date: update.date.map(|d| d.to_string()),
        })),
        None => Ok(None),
    }
}

/// Download, verify, and install the pending update, then relaunch. Call this
/// **only after the user has explicitly approved** in the UI. The updater
/// verifies the signature against the embedded public key before applying; a
/// failed verification surfaces as an error and nothing is installed.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Err("No update is available to install.".into());
    };
    update
        .download_and_install(|_downloaded, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    // Relaunch into the freshly installed version. `restart` does not return.
    app.restart();
}
