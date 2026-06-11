//! Guards the Rust ↔ JS IPC contract.
//!
//! The frontend is hand-written vanilla JS with no build step, so nothing
//! catches a typo'd or unregistered command until a button silently fails at
//! runtime. This test cross-checks every `invoke("…")` in src/main.js against
//! the commands registered in lib.rs's `generate_handler!`.

use std::collections::HashSet;
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
