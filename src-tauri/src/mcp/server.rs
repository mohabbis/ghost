//! Local stdio MCP server (JSON-RPC 2.0, newline-delimited).

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

use super::handlers;
use super::pairing::verify_pairing_code;

/// Run the MCP server on stdin/stdout until EOF.
pub fn run_stdio() {
    let stdin = BufReader::new(std::io::stdin());
    let mut stdout = std::io::stdout();

    for line in stdin.lines() {
        let Ok(line) = line else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = handle_message(trimmed);
        if let Ok(text) = serde_json::to_string(&response) {
            let _ = writeln!(stdout, "{text}");
            let _ = stdout.flush();
        }
    }
}

/// Handle one JSON-RPC request line/body and return the response object.
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

    if method == "notifications/initialized" || method.starts_with("notifications/") {
        return json!({ "jsonrpc": "2.0" });
    }

    let result = match method {
        "initialize" => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            let pairing_code = params
                .pointer("/capabilities/ghost/pairing_code")
                .or_else(|| params.get("pairing_code"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if !verify_pairing_code(pairing_code) {
                return json_rpc_error(id, -32001, "MCP pairing code is required or invalid");
            }
            Ok(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "ghost", "version": env!("CARGO_PKG_VERSION") },
            }))
        }
        "tools/list" => Ok(json!({ "tools": handlers::list_tools() })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            let name = params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or_default();
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match handlers::handle_tool(name, &args) {
                Ok(value) => Ok(json!({
                    "content": [{ "type": "text", "text": value.to_string() }],
                    "isError": false,
                })),
                Err(err) => Ok(json!({
                    "content": [{ "type": "text", "text": err }],
                    "isError": true,
                })),
            }
        }
        _ => Err(format!("Method not found: {method}")),
    };

    match result {
        Ok(result) => json_rpc_result(id, result),
        Err(err) => json_rpc_error(id, -32601, &err),
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

    #[test]
    fn initialize_returns_server_info() {
        let resp = handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        assert_eq!(resp["result"]["serverInfo"]["name"], "ghost");
    }

    #[test]
    fn tools_list_is_non_empty() {
        let resp = handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#);
        assert!(resp["result"]["tools"].as_array().unwrap().len() >= 5);
    }
}
