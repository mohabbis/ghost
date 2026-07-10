use crate::engine::GhostEngine;
use tauri::State;

/// Combined auth state for the frontend lock screen / onboarding.
#[derive(serde::Serialize)]
pub struct AuthStatus {
    pub configured: bool,
    pub unlocked: bool,
}

/// Reject IPC that mutates local state while the vault is locked.
///
/// Ghost is local-first: “login” is a machine password that unlocks an
/// Argon2id-wrapped AES-256-GCM data key. Mutating Organizer / workflow
/// commands must call this so a locked session cannot move files or rewrite
/// boundaries. Unconfigured installs stay usable (password is strongly
/// recommended in onboarding, not a hard gate on first launch). Read-only
/// preview commands may stay reachable either way.
pub(crate) fn require_unlocked(engine: &GhostEngine) -> Result<(), String> {
    let auth = engine.auth();
    if auth.is_configured() && !auth.is_unlocked() {
        return Err("Ghost is locked. Unlock with your local password to continue.".into());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tauri::Manager;

    /// A `GhostEngine` whose auth file lives under a unique temp path, wrapped
    /// in a mock Tauri app so these tests exercise the real `State<GhostEngine>`
    /// extraction the commands run under — never the real OS data directory,
    /// so a run can never read or overwrite a developer's actual local password.
    fn managed_test_app() -> tauri::App<tauri::test::MockRuntime> {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let auth_path = std::env::temp_dir().join(format!(
            "ghost_auth_cmd_test_{}_{}.json",
            std::process::id(),
            n
        ));

        let app = tauri::test::mock_app();
        app.manage(GhostEngine::with_auth_path(auth_path));
        app
    }

    #[test]
    fn fresh_engine_reports_unconfigured_and_unlocked() {
        // An unconfigured installation is always "unlocked" (no protection
        // requested yet, so workflow data stays in plaintext) — see
        // `AuthManager::is_unlocked`.
        let app = managed_test_app();
        let status = auth_status(app.state());
        assert!(!status.configured);
        assert!(status.unlocked);
    }

    #[test]
    fn require_unlocked_allows_unconfigured_installs() {
        // First-run / skip-password must still be able to create Zones and
        // organize folders. Lock only applies after a password exists.
        let app = managed_test_app();
        require_unlocked(&app.state::<GhostEngine>()).expect("unconfigured should pass");
    }

    #[test]
    fn require_unlocked_blocks_when_locked() {
        let app = managed_test_app();
        auth_setup("correct horse battery".into(), app.state()).expect("setup");
        auth_lock(app.state());
        let err = require_unlocked(&app.state::<GhostEngine>()).expect_err("locked must fail");
        assert!(err.to_lowercase().contains("locked"));
    }

    #[test]
    fn setup_configures_and_leaves_unlocked() {
        let app = managed_test_app();
        auth_setup("correct horse battery".into(), app.state()).expect("setup should succeed");

        let status = auth_status(app.state());
        assert!(status.configured);
        assert!(status.unlocked, "setup should leave the app unlocked");
    }

    #[test]
    fn lock_then_status_reports_configured_but_locked() {
        let app = managed_test_app();
        auth_setup("correct horse battery".into(), app.state()).expect("setup should succeed");

        auth_lock(app.state());

        let status = auth_status(app.state());
        assert!(status.configured, "lock must not erase the stored password");
        assert!(!status.unlocked);
    }

    #[test]
    fn unlock_with_wrong_password_returns_false_without_erroring() {
        let app = managed_test_app();
        auth_setup("correct horse battery".into(), app.state()).expect("setup should succeed");
        auth_lock(app.state());

        let result = auth_unlock("not the password".into(), app.state());
        assert_eq!(
            result,
            Ok(false),
            "a wrong password is a normal false, not a command error"
        );
        assert!(!auth_status(app.state()).unlocked);
    }

    #[test]
    fn unlock_with_correct_password_returns_true_and_unlocks() {
        let app = managed_test_app();
        auth_setup("correct horse battery".into(), app.state()).expect("setup should succeed");
        auth_lock(app.state());

        let result = auth_unlock("correct horse battery".into(), app.state());
        assert_eq!(result, Ok(true));
        assert!(auth_status(app.state()).unlocked);
    }
}
