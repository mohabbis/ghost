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

/// Historical placeholder prefix for the updater public key. The production
/// `tauri.conf.json` now ships a real minisign key (see `docs/auto-update.md`),
/// but forks or dev builds may scrub it back to the `REPLACE_WITH…` sentinel —
/// this check keeps auto-update inert in that case rather than surfacing
/// signature-verification noise to users.
const PUBKEY_PLACEHOLDER_PREFIX: &str = "REPLACE_WITH";

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

/// Whether a real updater public key is configured. Returns `false` when the
/// key is missing, empty, or still the `tauri.conf.json` placeholder — in which
/// case update checks/installs are intentional no-ops (auto-update is disabled
/// until the maintainer completes the one-time key setup). Reads config via a
/// JSON view so it stays correct across Tauri config-struct changes.
fn updater_configured<R: tauri::Runtime>(app: &AppHandle<R>) -> bool {
    serde_json::to_value(app.config())
        .ok()
        .and_then(|cfg| {
            cfg.get("plugins")
                .and_then(|p| p.get("updater"))
                .and_then(|u| u.get("pubkey"))
                .and_then(|k| k.as_str())
                .map(str::to_owned)
        })
        .map(|key| {
            let key = key.trim();
            !key.is_empty() && !key.starts_with(PUBKEY_PLACEHOLDER_PREFIX)
        })
        .unwrap_or(false)
}

/// Check the configured endpoint for a newer signed release. **Read-only**: it
/// downloads nothing and changes nothing, so it is safe to call on launch.
/// Returns `None` when the app is already current, or when auto-update is not
/// yet configured (no real signing key) so an unconfigured build stays quiet.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    if !updater_configured(&app) {
        return Ok(None);
    }
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
    if !updater_configured(&app) {
        return Err("Auto-update is not configured for this build.".into());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updater_configured_is_false_without_a_real_pubkey() {
        // A mock app carries no updater plugin config at all, which is the
        // same shape as a real build that still has the tauri.conf.json
        // placeholder: no configured key means auto-update must stay inert
        // rather than surface signature-verification noise.
        let app = tauri::test::mock_app();
        assert!(!updater_configured(app.handle()));
    }
}
