//! Local stdio MCP server (JSON-RPC 2.0, newline-delimited).
//!
//! Pairing is enforced as a **session gate**, not just an `initialize` check:
//! when a pairing code is configured, `tools/list` and `tools/call` are
//! refused until this session has passed `initialize` with the correct code.
//! A client that skips `initialize` gets `-32001`, never tool access.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};

use super::handlers;
use super::pairing::{
    PairingConfig, load_pairing_config, pairing_is_required_with, verify_pairing_code_with,
};

/// Environment variable through which an MCP client's launch config can hand
/// Ghost the pairing code. Standard clients (Claude Desktop, Cursor) support
/// an `env` block per configured server but cannot inject custom `initialize`
/// params, so this is the paste-ready path; explicit `initialize` params take
/// precedence when both are present.
pub const PAIRING_CODE_ENV: &str = "GHOST_MCP_PAIRING_CODE";

/// Per-connection MCP session state. The pairing config is snapshotted when
/// the session starts, so a code rotation applies to the *next* connection
/// rather than yanking an already-authorized session mid-conversation.
pub struct McpSession {
    pairing: PairingConfig,
    /// Pairing code supplied via the launch environment ([`PAIRING_CODE_ENV`]),
    /// used at `initialize` when the client sends no code in params.
    env_code: Option<String>,
    authenticated: bool,
}

impl McpSession {
    /// A session gated by the persisted pairing config.
    pub fn new() -> Self {
        Self::with_config_and_env_code(load_pairing_config(), std::env::var(PAIRING_CODE_ENV).ok())
    }

    /// A session gated by an explicit config (used by tests).
    pub fn with_config(pairing: PairingConfig) -> Self {
        Self::with_config_and_env_code(pairing, None)
    }

    /// A session gated by an explicit config and an explicit environment code
    /// (tests inject instead of mutating the process environment).
    pub fn with_config_and_env_code(pairing: PairingConfig, env_code: Option<String>) -> Self {
        // With pairing disabled there is nothing to authenticate against;
        // the session starts open, matching the documented default.
        let authenticated = !pairing_is_required_with(&pairing);
        Self {
            pairing,
            env_code,
            authenticated,
        }
    }

    /// Handle one JSON-RPC request line and return the response object,
    /// enforcing the pairing session gate.
    pub fn handle_message(&mut self, line: &str) -> Value {
        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return json_rpc_error(None, -32700, &format!("Parse error: {e}")),
        };

        let id = request.get("id").cloned();
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or_default();

        if method.starts_with("notifications/") {
            return json!({ "jsonrpc": "2.0" });
        }

        match method {
            "initialize" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                // Params win over the env fallback so a client that *can* send
                // custom initialize params is never overridden by stale env.
                let param_code = extract_pairing_code(&params);
                let pairing_code = if param_code.is_empty() {
                    self.env_code.as_deref().unwrap_or_default()
                } else {
                    param_code
                };
                if !verify_pairing_code_with(&self.pairing, pairing_code) {
                    return json_rpc_error(id, -32001, "MCP pairing code is required or invalid");
                }
                self.authenticated = true;
                json_rpc_result(id, initialize_result())
            }
            "tools/list" | "tools/call" if !self.authenticated => json_rpc_error(
                id,
                -32001,
                "Session not paired: call initialize with the Ghost pairing code first",
            ),
            "tools/list" | "tools/call" => dispatch_tools(method, &request, id),
            _ => json_rpc_error(id, -32601, &format!("Method not found: {method}")),
        }
    }
}

impl Default for McpSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the MCP server on stdin/stdout until EOF.
pub fn run_stdio() {
    let stdin = BufReader::new(std::io::stdin());
    let mut stdout = std::io::stdout();
    let mut session = McpSession::new();

    for line in stdin.lines() {
        let Ok(line) = line else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = session.handle_message(trimmed);
        if let Ok(text) = serde_json::to_string(&response) {
            let _ = writeln!(stdout, "{text}");
            let _ = stdout.flush();
        }
    }
}

/// Handle one JSON-RPC request without session state.
///
/// Used by the experimental HTTP/relay transports, which authenticate every
/// request with their own bearer/device tokens before it reaches this point —
/// there is no long-lived session to gate. The stdio path must NOT use this;
/// it goes through [`McpSession`] so pairing protects the whole session.
pub fn handle_message(line: &str) -> Value {
    let request: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return json_rpc_error(None, -32700, &format!("Parse error: {e}")),
    };

    let id = request.get("id").cloned();
    let method = request
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or_default();

    if method.starts_with("notifications/") {
        return json!({ "jsonrpc": "2.0" });
    }

    match method {
        "initialize" => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            let pairing_code = extract_pairing_code(&params);
            if !verify_pairing_code_with(&load_pairing_config(), pairing_code) {
                return json_rpc_error(id, -32001, "MCP pairing code is required or invalid");
            }
            json_rpc_result(id, initialize_result())
        }
        "tools/list" | "tools/call" => dispatch_tools(method, &request, id),
        _ => json_rpc_error(id, -32601, &format!("Method not found: {method}")),
    }
}

fn extract_pairing_code(params: &Value) -> &str {
    params
        .pointer("/capabilities/ghost/pairing_code")
        .or_else(|| params.get("pairing_code"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "ghost", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn dispatch_tools(method: &str, request: &Value, id: Option<Value>) -> Value {
    match method {
        "tools/list" => json_rpc_result(id, json!({ "tools": handlers::list_tools() })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            let name = params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or_default();
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let result = match handlers::handle_tool(name, &args) {
                Ok(value) => json!({
                    "content": [{ "type": "text", "text": value.to_string() }],
                    "isError": false,
                }),
                Err(err) => json!({
                    "content": [{ "type": "text", "text": err }],
                    "isError": true,
                }),
            };
            json_rpc_result(id, result)
        }
        _ => unreachable!("dispatch_tools only receives tools/* methods"),
    }
}

fn json_rpc_result(id: Option<Value>, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn json_rpc_error(id: Option<Value>, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paired_config(code: &str) -> PairingConfig {
        PairingConfig {
            enabled: true,
            code: code.into(),
        }
    }

    fn open_config() -> PairingConfig {
        PairingConfig::default()
    }

    #[test]
    fn initialize_returns_server_info() {
        let mut session = McpSession::with_config(open_config());
        let resp =
            session.handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        assert_eq!(resp["result"]["serverInfo"]["name"], "ghost");
    }

    #[test]
    fn tools_list_is_non_empty_when_pairing_disabled() {
        let mut session = McpSession::with_config(open_config());
        let resp =
            session.handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#);
        assert!(resp["result"]["tools"].as_array().unwrap().len() >= 5);
    }

    #[test]
    fn paired_session_denies_tools_without_initialize() {
        // The gap this gate closes: a client that skips initialize must not
        // get tool access when a pairing code is configured.
        let mut session = McpSession::with_config(paired_config("ABCD2345"));
        for method in ["tools/list", "tools/call"] {
            let resp = session.handle_message(&format!(
                r#"{{"jsonrpc":"2.0","id":9,"method":"{method}","params":{{}}}}"#
            ));
            assert_eq!(resp["error"]["code"], -32001, "{method} must be gated");
            assert!(resp.get("result").is_none());
        }
    }

    #[test]
    fn paired_session_denies_wrong_code_and_stays_locked() {
        let mut session = McpSession::with_config(paired_config("ABCD2345"));
        let resp = session.handle_message(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"pairing_code":"WRONG999"}}"#,
        );
        assert_eq!(resp["error"]["code"], -32001);
        // A failed initialize must not unlock the session.
        let resp =
            session.handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#);
        assert_eq!(resp["error"]["code"], -32001);
    }

    #[test]
    fn paired_session_unlocks_after_correct_initialize() {
        let mut session = McpSession::with_config(paired_config("ABCD2345"));
        let resp = session.handle_message(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"pairing_code":"ABCD2345"}}"#,
        );
        assert_eq!(resp["result"]["serverInfo"]["name"], "ghost");
        let resp =
            session.handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#);
        assert!(resp["result"]["tools"].as_array().unwrap().len() >= 5);
    }

    #[test]
    fn paired_session_accepts_code_in_capabilities_envelope() {
        let mut session = McpSession::with_config(paired_config("ABCD2345"));
        let resp = session.handle_message(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{"ghost":{"pairing_code":"ABCD2345"}}}}"#,
        );
        assert_eq!(resp["result"]["serverInfo"]["name"], "ghost");
    }

    #[test]
    fn paired_session_accepts_code_from_launch_environment() {
        // The paste-ready client config path: Claude/Cursor pass the code via
        // an env block because they can't send custom initialize params.
        let mut session = McpSession::with_config_and_env_code(
            paired_config("ABCD2345"),
            Some("ABCD2345".into()),
        );
        let resp =
            session.handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        assert_eq!(resp["result"]["serverInfo"]["name"], "ghost");
        let resp =
            session.handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#);
        assert!(resp["result"]["tools"].as_array().unwrap().len() >= 5);
    }

    #[test]
    fn explicit_initialize_param_beats_stale_env_code() {
        let mut session = McpSession::with_config_and_env_code(
            paired_config("ABCD2345"),
            Some("OLDCODE9".into()),
        );
        let resp = session.handle_message(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"pairing_code":"ABCD2345"}}"#,
        );
        assert_eq!(resp["result"]["serverInfo"]["name"], "ghost");
    }

    #[test]
    fn wrong_env_code_stays_locked() {
        let mut session = McpSession::with_config_and_env_code(
            paired_config("ABCD2345"),
            Some("WRONG999".into()),
        );
        let resp =
            session.handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        assert_eq!(resp["error"]["code"], -32001);
        let resp =
            session.handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#);
        assert_eq!(resp["error"]["code"], -32001);
    }

    #[test]
    fn unpaired_session_allows_tools_without_initialize() {
        // Pairing disabled is the documented default: any local client works.
        let mut session = McpSession::with_config(open_config());
        let resp =
            session.handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#);
        assert!(resp["result"]["tools"].as_array().unwrap().len() >= 5);
    }
}
