//! Guards the Rust ↔ JS IPC contract.
//!
//! The frontend is hand-written vanilla JS with no build step, so nothing
//! catches a typo'd or unregistered command until a button silently fails at
//! runtime. This test cross-checks every `invoke("…")` in frontend JS against
//! the commands registered in lib.rs's `generate_handler!`, and every invoke
//! argument key against the command's Rust parameter names (Tauri 2 matches
//! JS keys against the camelCased Rust names — a snake_case key either errors
//! with "invalid args" or, for `Option` params, is silently dropped).

use std::collections::{HashMap, HashSet};
use std::path::Path;

fn read(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(root.join(rel))
        .unwrap_or_else(|e| panic!("could not read {}: {}", rel, e))
}

fn read_command_sources() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let command_dir = root.join("src/commands");

    let mut sources = vec![read("src/commands.rs")];

    if command_dir.exists() {
        let mut files = std::fs::read_dir(&command_dir)
            .unwrap_or_else(|e| panic!("could not read {}: {}", command_dir.display(), e))
            .map(|entry| entry.expect("could not read command source entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect::<Vec<_>>();
        files.sort();

        sources.extend(files.into_iter().map(|path| {
            std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("could not read {}: {}", path.display(), e))
        }));
    }

    sources.join("\n")
}

fn parse_command_lines(block: &str) -> HashSet<String> {
    block
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("commands::")
                .map(|rest| rest.trim_end_matches(',').to_string())
        })
        .collect()
}

/// Every `commands::…` entry declared in lib.rs — stable block plus the
/// experimental macro block (present in source even when the feature is off).
fn registered_commands() -> HashSet<String> {
    parse_command_lines(&read("src/lib.rs"))
}

// Only consumed by `#[cfg(not(feature = "experimental"))]` tests below, so an
// `--features experimental` build sees it as unused — pre-existing, harmless.
#[cfg_attr(feature = "experimental", allow(dead_code))]
fn experimental_only_commands() -> HashSet<String> {
    let lib = read("src/lib.rs");
    let mut in_experimental = false;
    let mut cmds = HashSet::new();
    for line in lib.lines() {
        if line.contains("macro_rules! run_experimental_app") {
            in_experimental = true;
            continue;
        }
        if in_experimental {
            if line.trim() == "};" {
                break;
            }
            if let Some(rest) = line.trim().strip_prefix("commands::") {
                cmds.insert(rest.trim_end_matches(',').to_string());
            }
        }
    }
    cmds
}

const FRONTEND_JS: &[&str] = &["../src/main.js", "../src/compression-review.js"];

fn invoked_commands() -> Vec<String> {
    let re = regex::Regex::new(r#"invoke\(\s*"([a-z0-9_]+)""#).unwrap();
    let mut out = Vec::new();
    for path in FRONTEND_JS {
        let js = read(path);
        out.extend(
            re.captures_iter(&js)
                .map(|cap| cap[1].to_string())
                .collect::<Vec<_>>(),
        );
    }
    out
}

#[test]
fn frontend_invokes_only_registered_commands() {
    let registered = registered_commands();
    assert!(
        registered.len() > 30,
        "lib.rs parsing looks broken — only found {} commands",
        registered.len()
    );

    let missing: Vec<String> = invoked_commands()
        .into_iter()
        .filter(|name| !registered.contains(name))
        .collect();

    assert!(
        missing.is_empty(),
        "frontend JS invokes commands not registered in lib.rs: {:?}",
        missing
    );
}

/// Automatic post-recording paths must not call experimental IPC in stock builds.
#[cfg(not(feature = "experimental"))]
#[test]
fn stock_build_observer_learn_is_gated() {
    let js = read("../src/main.js");
    let re =
        regex::Regex::new(r"(?s)async function observerLearnFromSession\(\)\s*\{.*?\n\}").unwrap();
    let body = re
        .find(&js)
        .expect("observerLearnFromSession must exist")
        .as_str();
    assert!(
        body.contains("experimentalEnabled"),
        "observerLearnFromSession must gate on experimentalEnabled in stock builds"
    );
}

/// The Settings modal's AI Providers status fetch must stay gated in stock
/// builds — `intelligence_provider_status` is experimental-only.
#[cfg(not(feature = "experimental"))]
#[test]
fn stock_build_open_settings_gates_intelligence_status_call() {
    let js = read("../src/main.js");
    let re = regex::Regex::new(r"(?s)async function openSettings\(\)\s*\{.*?\n\}").unwrap();
    let body = re.find(&js).expect("openSettings must exist").as_str();
    let idx = body
        .find("intelligence_provider_status")
        .expect("openSettings must still reference intelligence_provider_status when gated");
    let before = &body[..idx];
    assert!(
        before.rfind("experimentalEnabled").is_some(),
        "openSettings must gate intelligence_provider_status on experimentalEnabled"
    );
}

/// Both `intelligence_set_api_key` calls in `saveSettings` must stay gated
/// in stock builds.
#[cfg(not(feature = "experimental"))]
#[test]
fn stock_build_save_settings_gates_intelligence_set_api_key_calls() {
    let js = read("../src/main.js");
    let re = regex::Regex::new(r"(?s)async function saveSettings\(\)\s*\{.*?\n\}").unwrap();
    let body = re.find(&js).expect("saveSettings must exist").as_str();
    let mut search_from = 0usize;
    let mut found = 0usize;
    while let Some(rel_idx) = body[search_from..].find("intelligence_set_api_key") {
        let idx = search_from + rel_idx;
        let before = &body[..idx];
        assert!(
            before.rfind("experimentalEnabled").is_some(),
            "saveSettings must gate every intelligence_set_api_key call on experimentalEnabled"
        );
        found += 1;
        search_from = idx + "intelligence_set_api_key".len();
    }
    assert_eq!(
        found, 2,
        "expected both the OpenAI and Anthropic intelligence_set_api_key calls in saveSettings"
    );
}

/// `testIntelligenceProvider` must refuse to call `intelligence_test_provider`
/// in stock builds.
#[cfg(not(feature = "experimental"))]
#[test]
fn stock_build_test_intelligence_provider_is_gated() {
    let js = read("../src/main.js");
    let re =
        regex::Regex::new(r"(?s)async function testIntelligenceProvider\(provider\)\s*\{.*?\n\}")
            .unwrap();
    let body = re
        .find(&js)
        .expect("testIntelligenceProvider must exist")
        .as_str();
    let idx = body.find("intelligence_test_provider").expect(
        "testIntelligenceProvider must still reference intelligence_test_provider when gated",
    );
    let before = &body[..idx];
    assert!(
        before.rfind("experimentalEnabled").is_some(),
        "testIntelligenceProvider must gate intelligence_test_provider on experimentalEnabled"
    );
}

/// The four Power BI export functions must each refuse to call their
/// experimental-only command in stock builds — same pattern as
/// `stock_build_observer_learn_is_gated`.
#[cfg(not(feature = "experimental"))]
#[test]
fn stock_build_power_bi_functions_are_gated() {
    let js = read("../src/main.js");
    let functions = [
        "connectPowerBi",
        "previewPowerBiExport",
        "pushPowerBiExport",
        "revokePowerBiGrant",
    ];
    for name in functions {
        let re =
            regex::Regex::new(&format!(r"(?s)async function {name}\(\)\s*\{{.*?\n\}}")).unwrap();
        let body = re
            .find(&js)
            .unwrap_or_else(|| panic!("{name} must exist"))
            .as_str();
        assert!(
            body.contains("experimentalEnabled"),
            "{name} must gate on experimentalEnabled in stock builds"
        );
    }
}

/// `openSettings` must gate experimental MCP/Fabric status fetches in stock builds.
#[cfg(not(feature = "experimental"))]
#[test]
fn stock_build_open_settings_gates_mcp_status_calls() {
    let js = read("../src/main.js");
    let re = regex::Regex::new(r"(?s)async function openSettings\(\)\s*\{.*?\n\}").unwrap();
    let body = re.find(&js).expect("openSettings must exist").as_str();
    for cmd in [
        "mcp_http_server_status",
        "mcp_relay_status",
        "fabric_webhook_status",
    ] {
        let idx = body
            .find(cmd)
            .unwrap_or_else(|| panic!("openSettings must reference {cmd} when gated"));
        let before = &body[..idx];
        assert!(
            before.rfind("experimentalEnabled").is_some(),
            "openSettings must gate {cmd} on experimentalEnabled"
        );
    }
}

/// MCP HTTP/relay controls must refuse experimental IPC in stock builds.
#[cfg(not(feature = "experimental"))]
#[test]
fn stock_build_mcp_http_and_relay_functions_are_gated() {
    let js = read("../src/main.js");
    let functions = [
        "startMcpHttpServer",
        "stopMcpHttpServer",
        "startMcpRelay",
        "stopMcpRelay",
        "generateFabricWebhookSecret",
        "bindGoogleExportBucket",
    ];
    for name in functions {
        let re =
            regex::Regex::new(&format!(r"(?s)async function {name}\(\)\s*\{{.*?\n\}}")).unwrap();
        let body = re
            .find(&js)
            .unwrap_or_else(|| panic!("{name} must exist"))
            .as_str();
        assert!(
            body.contains("experimentalEnabled"),
            "{name} must gate on experimentalEnabled in stock builds"
        );
    }
}

#[cfg(not(feature = "experimental"))]
#[test]
fn stock_build_compression_review_js_has_no_experimental_invokes() {
    let experimental = experimental_only_commands();
    let js = read("../src/compression-review.js");
    let re = regex::Regex::new(r#"invoke\(\s*"([a-z0-9_]+)""#).unwrap();
    let bad: Vec<String> = re
        .captures_iter(&js)
        .map(|cap| cap[1].to_string())
        .filter(|name| experimental.contains(name))
        .collect();
    assert!(
        bad.is_empty(),
        "compression-review.js must not invoke experimental-only commands: {:?}",
        bad
    );
}

/// camelCase a snake_case Rust identifier the way Tauri 2 does for invoke args.
fn camel_case(s: &str) -> String {
    let mut out = String::new();
    let mut upper_next = false;
    for c in s.chars() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(c.to_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Split a string on `sep` at bracket depth 0 (so generic types and nested
/// object literals don't get cut in half).
fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for c in s.chars() {
        match c {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if c == sep && depth == 0 {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    parts.push(current);
    parts
}

/// Parse command sources into command → set of accepted JS arg keys (camelCased
/// param names, minus Tauri-injected params like State/AppHandle/Window).
fn command_arg_keys() -> HashMap<String, HashSet<String>> {
    let src = read_command_sources();
    let re = regex::Regex::new(
        r"(?s)(?:#\[cfg\([^\)]*\)\]\s*)*#\[tauri::command\]\s*pub (?:async )?fn (\w+)\s*\((.*?)\)\s*(?:->|where|\{)",
    )
    .unwrap();
    let mut map = HashMap::new();
    for cap in re.captures_iter(&src) {
        let params: String = cap[2]
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        let mut keys = HashSet::new();
        for param in split_top_level(&params, ',') {
            let param = param.trim();
            let Some((name, ty)) = param.split_once(':') else {
                continue;
            };
            if ty.contains("State<") || ty.contains("AppHandle") || ty.contains("Window") {
                continue;
            }
            keys.insert(camel_case(name.trim()));
        }
        map.insert(cap[1].to_string(), keys);
    }
    map
}

/// Extract the body of the object literal that starts at `open_brace`
/// (an index of `{` in `js`), handling nested braces and string literals.
fn object_literal_body(js: &str, open_brace: usize) -> Option<&str> {
    let bytes = js.as_bytes();
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    let mut i = open_brace;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(quote) = in_str {
            if c == b'\\' {
                i += 1;
            } else if c == quote {
                in_str = None;
            }
        } else {
            match c {
                b'"' | b'\'' | b'`' => in_str = Some(c),
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&js[open_brace + 1..i]);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Top-level keys of a JS object literal body: `key: value` entries and
/// `{ shorthand }` entries (where the key IS the sent identifier).
fn object_keys(body: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for entry in split_top_level(body, ',') {
        let entry = entry.trim();
        if entry.is_empty() || entry.starts_with("...") {
            continue;
        }
        let key = match split_top_level(entry, ':').first() {
            Some(k) => k.trim().to_string(),
            None => continue,
        };
        if key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
            && !key.is_empty()
        {
            keys.push(key);
        }
    }
    keys
}

/// Every `invoke("cmd", { … })` in frontend JS with its top-level arg keys.
fn invoked_with_args() -> Vec<(String, Vec<String>)> {
    let re = regex::Regex::new(r#"invoke\(\s*"([a-z0-9_]+)"\s*,\s*\{"#).unwrap();
    let mut calls = Vec::new();
    for path in FRONTEND_JS {
        let js = read(path);
        calls.extend(re.captures_iter(&js).filter_map(|cap| {
            let open = cap.get(0).unwrap().end() - 1;
            object_literal_body(&js, open).map(|body| (cap[1].to_string(), object_keys(body)))
        }));
    }
    calls
}

#[test]
fn frontend_invoke_args_match_command_params() {
    let commands = command_arg_keys();
    assert!(
        commands.len() > 30,
        "command source parsing looks broken — only found {} commands",
        commands.len()
    );

    let calls = invoked_with_args();
    assert!(
        calls.len() > 10,
        "frontend arg parsing looks broken — only found {} invocations with args",
        calls.len()
    );

    let mut problems = Vec::new();
    for (cmd, keys) in calls {
        let Some(expected) = commands.get(&cmd) else {
            continue;
        };
        for key in keys {
            if !expected.contains(&key) {
                problems.push(format!(
                    "invoke(\"{}\") sends key `{}` but the Rust command accepts {:?} \
                     (Tauri matches camelCased param names)",
                    cmd, key, expected
                ));
            }
        }
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn frontend_actually_uses_the_ipc_bridge() {
    let invoked = invoked_commands();
    assert!(
        invoked.len() > 20,
        "frontend invoke parsing looks broken — only found {} invocations",
        invoked.len()
    );
}

/// Experimental MCP HTTP/relay commands must accept the camelCased keys the
/// Settings UI sends (Tauri 2 matches JS keys to Rust param names).
#[cfg(feature = "experimental")]
#[test]
fn experimental_mcp_http_and_relay_ipc_arg_shapes() {
    use std::collections::HashSet;

    let cmds = command_arg_keys();
    let http = cmds
        .get("mcp_start_http_server")
        .expect("mcp_start_http_server must be parsed from commands/mcp.rs");
    assert_eq!(
        http,
        &HashSet::from([
            "port".to_string(),
            "exposeLan".to_string(),
            "bearerToken".to_string(),
            "tlsCertPath".to_string(),
            "tlsKeyPath".to_string(),
        ])
    );

    let relay = cmds
        .get("mcp_start_relay")
        .expect("mcp_start_relay must be parsed from commands/mcp.rs");
    assert_eq!(
        relay,
        &HashSet::from([
            "relayUrl".to_string(),
            "deviceId".to_string(),
            "deviceToken".to_string(),
        ])
    );
}

/// Frontend invoke keys for experimental MCP commands must match the Rust
/// command signatures exactly.
#[cfg(feature = "experimental")]
#[test]
fn experimental_frontend_mcp_invoke_args_match_command_params() {
    let commands = command_arg_keys();
    let expected_cmds = ["mcp_start_http_server", "mcp_start_relay"];
    let calls: Vec<_> = invoked_with_args()
        .into_iter()
        .filter(|(cmd, _)| expected_cmds.contains(&cmd.as_str()))
        .collect();
    assert!(
        calls.len() >= 2,
        "expected frontend invocations for mcp_start_http_server and mcp_start_relay, found {:?}",
        calls.iter().map(|(c, _)| c).collect::<Vec<_>>()
    );

    let mut problems = Vec::new();
    for (cmd, keys) in calls {
        let Some(expected) = commands.get(&cmd) else {
            problems.push(format!("{cmd} not found in command source parse"));
            continue;
        };
        for key in keys {
            if !expected.contains(&key) {
                problems.push(format!(
                    "invoke(\"{cmd}\") sends key `{key}` but the Rust command accepts {expected:?}"
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Experimental MCP relay/HTTP commands must be registered in lib.rs.
#[cfg(feature = "experimental")]
#[test]
fn experimental_build_registers_mcp_relay_and_http_commands() {
    let registered = registered_commands();
    for cmd in [
        "mcp_http_server_status",
        "mcp_start_http_server",
        "mcp_stop_http_server",
        "mcp_relay_status",
        "mcp_start_relay",
        "mcp_stop_relay",
    ] {
        assert!(
            registered.contains(cmd),
            "{cmd} must be registered in lib.rs generate_handler!"
        );
    }
}

/// Account sign-in is stable core — `account_status` must load unconditionally in
/// Settings, not behind the experimental feature gate.
#[cfg(not(feature = "experimental"))]
#[test]
fn stock_build_open_settings_calls_account_status_unconditionally() {
    let js = read("../src/main.js");
    let re = regex::Regex::new(r"(?s)async function openSettings\(\)\s*\{.*?\n\}").unwrap();
    let body = re.find(&js).expect("openSettings must exist").as_str();
    let idx = body
        .find("account_status")
        .expect("openSettings must invoke account_status");
    let before = &body[..idx];
    assert!(
        !before.contains("if (experimentalEnabled)"),
        "account_status must not be gated on experimentalEnabled"
    );
}

#[test]
fn open_settings_account_section_uses_availability_fields() {
    let js = read("../src/main.js");
    let re = regex::Regex::new(r"(?s)async function openSettings\(\)\s*\{.*?\n\}").unwrap();
    let body = re.find(&js).expect("openSettings must exist").as_str();
    for marker in [
        "google_sign_in_available",
        "microsoft_sign_in_available",
        "data-account-sign-in=\"google\"",
        "data-account-sign-in=\"microsoft\"",
        "account-status-note",
        "data-account-sign-out",
    ] {
        assert!(
            body.contains(marker),
            "openSettings must render account availability wiring `{marker}`"
        );
    }
}

#[test]
fn sign_in_with_provider_guards_disabled_buttons() {
    let js = read("../src/main.js");
    let re = regex::Regex::new(r"(?s)async function signInWithProvider\(provider\)\s*\{.*?\n\}")
        .unwrap();
    let body = re
        .find(&js)
        .expect("signInWithProvider must exist")
        .as_str();
    assert!(
        body.contains("btn?.disabled"),
        "signInWithProvider must refuse disabled provider buttons"
    );
    assert!(
        body.contains("account-status-note"),
        "signInWithProvider must surface availability hints on account-status-note"
    );
    let invoke_idx = body
        .find("account_sign_in")
        .expect("must call account_sign_in");
    let disabled_idx = body
        .find("btn?.disabled")
        .expect("must check disabled before invoke");
    assert!(
        disabled_idx < invoke_idx,
        "signInWithProvider must check btn?.disabled before account_sign_in"
    );
}

#[test]
fn compression_review_js_invokes_routine_policy_plan() {
    let js = read("../src/compression-review.js");
    assert!(
        js.contains("routine_policy_plan"),
        "compression-review.js must fetch routine_policy_plan for the review timeline"
    );
}

#[test]
fn domcontentloaded_checks_replay_unfinished_run() {
    let js = read("../src/main.js");
    let dom_ready = js
        .find("window.addEventListener(\"DOMContentLoaded\"")
        .map(|idx| &js[idx..])
        .expect("DOMContentLoaded bootstrap must exist");
    assert!(
        dom_ready.contains("replayCheckUnfinishedRun"),
        "app bootstrap must check for interrupted replay runs"
    );
}

#[test]
fn replay_workflow_still_goes_through_policy_approval() {
    let js = read("../src/main.js");
    let re = regex::Regex::new(r"(?s)async function replayWorkflow\(\)\s*\{.*?\n\}").unwrap();
    let body = re.find(&js).expect("replayWorkflow must exist").as_str();
    assert!(
        body.contains("confirmPolicyBeforeReplay"),
        "replayWorkflow must gate on confirmPolicyBeforeReplay"
    );
    let confirm_idx = body.find("confirmPolicyBeforeReplay").unwrap();
    let replay_idx = body
        .find("execute_routine_action_plan")
        .expect("replayWorkflow must invoke execute_routine_action_plan");
    assert!(
        confirm_idx < replay_idx,
        "confirmPolicyBeforeReplay must run before execute_routine_action_plan"
    );
}

#[test]
fn replay_history_surfaces_persisted_routine_receipts() {
    let js = read("../src/main.js");
    let start = js
        .find("async function showReplayHistory()")
        .expect("showReplayHistory must exist");
    let end = js[start..]
        .find("// ===== Replay inspection =====")
        .map(|offset| start + offset)
        .expect("Replay inspection section must follow showReplayHistory");
    let body = &js[start..end];

    for marker in [
        r#"invoke("get_replay_history""#,
        r#"invoke("organizer_list_executions""#,
        r#"run.zone_id === "routine""#,
        r#"invoke("get_execution_receipt""#,
        "data-replay-receipt-exec",
        "Verification receipts",
    ] {
        assert!(
            body.contains(marker),
            "Replay History must include persisted verification-receipt marker `{marker}`"
        );
    }
}
