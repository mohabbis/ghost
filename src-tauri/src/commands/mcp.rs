//! MCP pairing and Organizer approval-token commands (stable core).
//!
//! Command: `organizer_issue_mcp_approval_token` | risk: `safe-read` | issues a
//! short-lived signed token after the user reviews a plan in the desktop UI.
//!
//! Command: `mcp_pairing_status` | risk: `safe-read`
//! Command: `mcp_enable_pairing` | risk: `local-mutate`
//! Command: `mcp_disable_pairing` | risk: `local-mutate`

use crate::engine::GhostEngine;
use crate::mcp::{hash_organizer_plan, issue_approval_token, pairing, pending};
use crate::organizer::planner::plan_zone;
use crate::storage::open_default;
use tauri::State;

#[derive(serde::Serialize)]
pub struct McpPairingStatus {
    pub enabled: bool,
    pub code_hint: Option<String>,
}

/// Whether local MCP clients must supply a pairing code on `initialize`.
#[tauri::command]
pub fn mcp_pairing_status() -> McpPairingStatus {
    let cfg = pairing::pairing_status();
    McpPairingStatus {
        enabled: cfg.enabled,
        code_hint: if cfg.enabled && !cfg.code.is_empty() {
            Some(cfg.code)
        } else {
            None
        },
    }
}

#[derive(serde::Serialize)]
pub struct McpPairingEnableResult {
    pub code: String,
}

/// Generate a new pairing code. User must copy it into MCP client config.
#[tauri::command]
pub fn mcp_enable_pairing() -> McpPairingEnableResult {
    McpPairingEnableResult {
        code: pairing::enable_pairing(),
    }
}

/// Turn off MCP pairing (stdio clients can connect without a code).
#[tauri::command]
pub fn mcp_disable_pairing() {
    pairing::disable_pairing();
}

/// Pending MCP approval requests waiting for desktop review.
#[tauri::command]
pub fn mcp_list_pending_approvals() -> Vec<pending::PendingApprovalRequest> {
    pending::list_pending()
}

/// Issue a signed, short-lived approval token for the current server-side plan.
/// The user must have reviewed the plan in the Organizer UI first.
#[tauri::command]
pub fn organizer_issue_mcp_approval_token(
    zone_id: String,
    engine: State<GhostEngine>,
) -> Result<String, String> {
    super::auth::require_unlocked(&engine)?;
    let conn = open_default().map_err(|e| e.to_string())?;
    let plan = plan_zone(&conn, &zone_id).map_err(|e| e.to_string())?;
    let plan_hash = hash_organizer_plan(&plan);
    let signed = issue_approval_token(&format!("plan_{zone_id}"), &plan_hash);
    pending::mark_approved_for_zone(&zone_id);
    serde_json::to_string(&signed).map_err(|e| e.to_string())
}

#[cfg(feature = "experimental")]
#[tauri::command]
pub fn mcp_http_server_status() -> crate::mcp::HttpServerStatus {
    crate::mcp::http_server_status()
}

#[cfg(feature = "experimental")]
#[tauri::command]
pub fn mcp_start_http_server(
    port: Option<u16>,
    expose_lan: Option<bool>,
    bearer_token: Option<String>,
    tls_cert_path: Option<String>,
    tls_key_path: Option<String>,
) -> Result<crate::mcp::HttpServerStatus, String> {
    let webhook_secret = crate::integrations::microsoft::fabric::webhook::current_secret();
    let options = crate::mcp::HttpServerOptions {
        bind_host: if expose_lan.unwrap_or(false) {
            "0.0.0.0".to_string()
        } else {
            "127.0.0.1".to_string()
        },
        port: port.unwrap_or(8787),
        bearer_token,
        fabric_webhook_secret: webhook_secret,
        tls_cert_path,
        tls_key_path,
    };
    crate::mcp::start_http_server(options)?;
    Ok(crate::mcp::http_server_status())
}

#[cfg(feature = "experimental")]
#[tauri::command]
pub fn mcp_stop_http_server() -> crate::mcp::HttpServerStatus {
    crate::mcp::stop_http_server();
    crate::mcp::http_server_status()
}

#[cfg(feature = "experimental")]
#[tauri::command]
pub fn mcp_relay_status() -> crate::mcp::RelayStatus {
    crate::mcp::relay_status()
}

#[cfg(feature = "experimental")]
#[tauri::command]
pub fn mcp_start_relay(
    relay_url: String,
    device_id: String,
    device_token: String,
) -> Result<crate::mcp::RelayStatus, String> {
    crate::mcp::start_relay(crate::mcp::RelayOptions {
        relay_url,
        device_id,
        device_token,
    })?;
    Ok(crate::mcp::relay_status())
}

#[cfg(feature = "experimental")]
#[tauri::command]
pub fn mcp_stop_relay() -> crate::mcp::RelayStatus {
    crate::mcp::stop_relay();
    crate::mcp::relay_status()
}
