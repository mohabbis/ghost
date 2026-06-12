//! Guards the Rust ↔ JS IPC contract.
//!
//! The frontend is hand-written vanilla JS with no build step, so nothing
//! catches a typo'd or unregistered command until a button silently fails at
//! runtime. This test cross-checks every `invoke("…")` in src/main.js against
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

fn registered_commands() -> HashSet<String> {
    read("src/lib.rs")
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("commands::")
                .map(|rest| rest.trim_end_matches(',').to_string())
        })
        .collect()
}

fn invoked_commands() -> Vec<String> {
    let js = read("../src/main.js");
    let re = regex::Regex::new(r#"invoke\(\s*"([a-z0-9_]+)""#).unwrap();
    re.captures_iter(&js)
        .map(|cap| cap[1].to_string())
        .collect()
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
        "src/main.js invokes commands not registered in lib.rs: {:?}",
        missing
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

/// Parse commands.rs into command → set of accepted JS arg keys (camelCased
/// param names, minus Tauri-injected params like State/AppHandle/Window).
fn command_arg_keys() -> HashMap<String, HashSet<String>> {
    let src = read("src/commands.rs");
    let re = regex::Regex::new(r"(?s)#\[tauri::command\]\s*pub (?:async )?fn (\w+)\s*\(([^)]*)\)")
        .unwrap();
    let mut map = HashMap::new();
    for cap in re.captures_iter(&src) {
        // Strip `// …` line comments so commas inside them don't split params.
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
                i += 1; // skip escaped char
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

/// Every `invoke("cmd", { … })` in main.js with its top-level arg keys.
fn invoked_with_args() -> Vec<(String, Vec<String>)> {
    let js = read("../src/main.js");
    let re = regex::Regex::new(r#"invoke\(\s*"([a-z0-9_]+)"\s*,\s*\{"#).unwrap();
    re.captures_iter(&js)
        .filter_map(|cap| {
            let open = cap.get(0).unwrap().end() - 1;
            object_literal_body(&js, open).map(|body| (cap[1].to_string(), object_keys(body)))
        })
        .collect()
}

#[test]
fn frontend_invoke_args_match_command_params() {
    let commands = command_arg_keys();
    assert!(
        commands.len() > 30,
        "commands.rs parsing looks broken — only found {} commands",
        commands.len()
    );

    let calls = invoked_with_args();
    assert!(
        calls.len() > 10,
        "main.js arg parsing looks broken — only found {} invocations with args",
        calls.len()
    );

    let mut problems = Vec::new();
    for (cmd, keys) in calls {
        let Some(expected) = commands.get(&cmd) else {
            // Unregistered commands are caught by the test above.
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
    // Sanity check that the regex is matching real code, so the contract
    // test above can't silently pass on zero matches.
    let invoked = invoked_commands();
    assert!(
        invoked.len() > 20,
        "main.js parsing looks broken — only found {} invocations",
        invoked.len()
    );
}
