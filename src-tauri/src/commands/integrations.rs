//! Power BI audit-export: grant flow, preview, and push. Gated behind
//! `--features experimental` (see `lib.rs`) — a brand-new, unproven,
//! real network-write to a third-party paid service, closer in kind to
//! `commands/experimental.rs`'s `cloud_authenticate`/`cloud_sync_workflows`
//! ("sends local data outward") than to the identity-only `account_sign_in`.
//!
//! Command: `power_bi_grant_status` | risk: `safe-read` | reads: local grant
//! metadata only | mutates: no | network: no | approval: none.
//!
//! Command: `power_bi_request_grant` | risk: `external-mutate` | mutates:
//! writes a new integration grant | network: yes (opens the system browser,
//! then Microsoft's own token endpoint — no Power BI API call yet) |
//! approval: implicit in the user clicking "Connect Power BI" and completing
//! Microsoft's consent screen | requires an existing signed-in identity
//! (`account_sign_in` first).
//!
//! Command: `power_bi_revoke_grant` | risk: `local-mutate` | mutates: revokes
//! the local grant record | network: no (local only; does not revoke
//! provider-side consent).
//!
//! Command: `power_bi_export_preview` | risk: `safe-read` | pure, read-only:
//! assembles the exact payload a push would send, no network, so the user
//! can review it before ever pushing.
//!
//! Command: `power_bi_push_audit_export` | risk: `external-mutate` | mutates:
//! remote state (a Power BI dataset) | network: yes | requires an active
//! Power BI grant | re-derives the export payload server-side rather than
//! trusting anything the frontend supplied — the same "never trust a stale
//! client-supplied plan" principle `organizer_execute` already follows for
//! Organizer plans.

use crate::engine::GhostEngine;
use crate::identity::IntegrationKind;
use crate::integrations::microsoft::power_bi::export::{build_export, AuditExportPayload};
use crate::integrations::microsoft::power_bi::{schema, PowerBiClient};
use crate::integrations::microsoft::MicrosoftIntegrationService;
use crate::storage::executions::list_full_executions_since;
use crate::storage::open_default;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

/// The single push dataset Ghost targets in "My workspace" (v1 has no
/// workspace/dataset picker — see `docs/power-bi-integration.md`).
const DATASET_NAME: &str = "GhostOperations";

fn service(engine: &GhostEngine) -> MicrosoftIntegrationService {
    MicrosoftIntegrationService::new(engine.accounts().identity_store().clone())
}

/// `since_days` of `None` means "all local history" (cutoff 0); otherwise the
/// cutoff is `now - since_days` days, in epoch seconds.
fn since_epoch_secs(since_days: Option<u32>) -> u64 {
    let Some(days) = since_days else { return 0 };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.saturating_sub(u64::from(days).saturating_mul(86_400))
}

#[derive(serde::Serialize, Default)]
pub struct PowerBiGrantStatus {
    pub active: bool,
    pub granted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Whether a Power BI grant is currently active, and when it was granted.
#[tauri::command]
pub fn power_bi_grant_status(engine: State<GhostEngine>) -> PowerBiGrantStatus {
    let auth = engine.auth();
    let grant = engine
        .accounts()
        .identity_store()
        .active_grants(&auth)
        .into_iter()
        .find(|g| g.integration == IntegrationKind::MicrosoftPowerBi);
    match grant {
        Some(g) => PowerBiGrantStatus {
            active: true,
            granted_at: Some(g.granted_at),
            expires_at: g.expires_at,
        },
        None => PowerBiGrantStatus::default(),
    }
}

/// Run incremental consent for Power BI and persist the resulting grant.
/// Requires an existing signed-in identity. Blocks on browser/network
/// interaction, so it runs on a blocking thread rather than the async
/// command's own task — same pattern as `commands/account.rs::account_sign_in`.
#[tauri::command]
pub async fn power_bi_request_grant(
    engine: State<'_, GhostEngine>,
) -> Result<PowerBiGrantStatus, String> {
    let config = engine.get_config();
    let client_id = crate::identity::OAuthProvider::Microsoft.client_id(&config.integrations)?;
    let auth = engine.auth();
    let svc = service(&engine);

    tauri::async_runtime::spawn_blocking(move || svc.request_power_bi_grant(&auth, &client_id))
        .await
        .map_err(|e| format!("grant request task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    Ok(power_bi_grant_status(engine))
}

/// Revoke the local Power BI grant. Does not contact Microsoft.
#[tauri::command]
pub fn power_bi_revoke_grant(engine: State<GhostEngine>) -> Result<(), String> {
    let auth = engine.auth();
    service(&engine)
        .revoke_power_bi_grant(&auth)
        .map_err(|e| e.to_string())
}

/// Pure, read-only preview of exactly what `power_bi_push_audit_export`
/// would send — no network call. `since_days` bounds the export window
/// (omit for all local history).
#[tauri::command]
pub fn power_bi_export_preview(
    since_days: Option<u32>,
    engine: State<GhostEngine>,
) -> Result<AuditExportPayload, String> {
    let _ = &engine; // kept for signature symmetry with the push command
    let conn = open_default().map_err(|e| e.to_string())?;
    export_preview_with_conn(&conn, since_days)
}

/// The actual logic behind `power_bi_export_preview`, taking an already-open
/// connection so it is testable against an in-memory database instead of the
/// real OS data directory (`open_default` has no test seam of its own) —
/// mirrors `commands/organizer.rs`'s `organizer_plan_with_conn` pattern.
fn export_preview_with_conn(
    conn: &Connection,
    since_days: Option<u32>,
) -> Result<AuditExportPayload, String> {
    let executions = list_full_executions_since(conn, since_epoch_secs(since_days))
        .map_err(|e| e.to_string())?;
    Ok(build_export(&executions))
}

#[derive(serde::Serialize)]
pub struct PowerBiPushResult {
    pub runs_pushed: usize,
    pub actions_pushed: usize,
    pub policy_events_pushed: usize,
}

/// Push the audit export to Power BI. Requires an active grant. Re-derives
/// the export payload from local execution history rather than trusting
/// anything the frontend supplied, so a push always reflects the live data
/// — never a stale or tampered client-side snapshot.
#[tauri::command]
pub async fn power_bi_push_audit_export(
    since_days: Option<u32>,
    engine: State<'_, GhostEngine>,
) -> Result<PowerBiPushResult, String> {
    let auth = engine.auth();
    let svc = service(&engine);
    let access_token = svc
        .power_bi_access_token(&auth)
        .map_err(|e| e.to_string())?;

    let conn = open_default().map_err(|e| e.to_string())?;
    let executions = list_full_executions_since(&conn, since_epoch_secs(since_days))
        .map_err(|e| e.to_string())?;
    let payload = build_export(&executions);

    let (runs_len, actions_len, policy_len) = (
        payload.runs.len(),
        payload.actions.len(),
        payload.policy_events.len(),
    );

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let client = PowerBiClient::new();
        let dataset_id = client
            .ensure_dataset(&access_token, DATASET_NAME)
            .map_err(|e| e.to_string())?;
        client
            .push_rows(
                &access_token,
                &dataset_id,
                schema::GHOST_RUNS_TABLE,
                &payload.runs,
            )
            .map_err(|e| e.to_string())?;
        client
            .push_rows(
                &access_token,
                &dataset_id,
                schema::GHOST_ACTIONS_TABLE,
                &payload.actions,
            )
            .map_err(|e| e.to_string())?;
        client
            .push_rows(
                &access_token,
                &dataset_id,
                schema::GHOST_POLICY_EVENTS_TABLE,
                &payload.policy_events,
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| format!("push task failed: {e}"))??;

    Ok(PowerBiPushResult {
        runs_pushed: runs_len,
        actions_pushed: actions_len,
        policy_events_pushed: policy_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tauri::Manager;

    fn managed_test_app() -> tauri::App<tauri::test::MockRuntime> {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let auth_path = std::env::temp_dir().join(format!(
            "ghost_integrations_cmd_test_{}_{}.json",
            std::process::id(),
            n
        ));
        let app = tauri::test::mock_app();
        app.manage(GhostEngine::with_auth_path(auth_path));
        app
    }

    #[test]
    fn power_bi_grant_status_reports_none_without_a_grant() {
        let app = managed_test_app();
        let status = power_bi_grant_status(app.state());
        assert!(!status.active);
        assert!(status.granted_at.is_none());
    }

    #[test]
    fn power_bi_export_preview_is_read_only_and_returns_empty_for_no_executions() {
        let conn = crate::storage::open_in_memory().unwrap();
        let payload = export_preview_with_conn(&conn, None).unwrap();
        assert!(payload.is_empty());
    }

    #[test]
    fn power_bi_push_denied_without_active_grant() {
        let app = managed_test_app();
        let engine = app.state::<GhostEngine>();
        let auth = engine.auth();
        // No grant stored — power_bi_access_token must fail before any
        // network call is attempted.
        let err = service(&engine).power_bi_access_token(&auth).unwrap_err();
        assert_eq!(
            err,
            crate::identity::IntegrationError::AuthenticationRequired
        );
    }
}
