//! Guards the JS -> DOM wiring contract for the no-build-step frontend.
//!
//! `src/main.js` reaches into the DOM almost entirely through
//! `document.getElementById("literal-id")`. Because there is no bundler and no
//! DOM test harness, a renamed or typo'd element id fails silently at runtime
//! (the lookup returns `null` and the feature just does nothing). This test
//! makes that wiring explicit: every literal id the JS looks up must be
//! *authored* somewhere the app can produce it — either a static `id="…"` in
//! `src/index.html` or an `id="…"` inside a template string in `src/main.js`
//! (elements the JS builds and injects at runtime).
//!
//! It is the DOM counterpart to `ipc_contract.rs` (which guards JS -> Rust IPC).

use std::collections::HashSet;
use std::path::Path;

fn read(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(root.join(rel))
        .unwrap_or_else(|e| panic!("could not read {}: {}", rel, e))
}

/// Every `document.getElementById("id")` (or `getElementById('id')`) literal in
/// main.js. Template-literal lookups (`getElementById(`x${i}`)`) are ignored on
/// purpose — they are not a static contract.
fn referenced_ids(js: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"getElementById\(\s*["']([A-Za-z0-9_-]+)["']\s*\)"#).unwrap();
    re.captures_iter(js).map(|c| c[1].to_string()).collect()
}

/// Every id authored in a source string: `id="…"` or `id='…'`. Applied to both
/// index.html (static markup) and main.js (runtime-injected markup).
fn authored_ids(src: &str) -> HashSet<String> {
    let re = regex::Regex::new(r#"\bid\s*=\s*["']([A-Za-z0-9_-]+)["']"#).unwrap();
    re.captures_iter(src).map(|c| c[1].to_string()).collect()
}

#[test]
fn frontend_getelementbyid_targets_are_authored() {
    let js = read("../src/main.js");
    let html = read("../src/index.html");

    let mut authored = authored_ids(&html);
    authored.extend(authored_ids(&js));

    let referenced = referenced_ids(&js);
    assert!(
        referenced.len() > 40,
        "getElementById parsing looks broken — only found {} references",
        referenced.len()
    );

    let missing: Vec<String> = referenced
        .into_iter()
        .filter(|id| !authored.contains(id))
        .collect();

    assert!(
        missing.is_empty(),
        "src/main.js calls getElementById for ids that are never authored in \
         src/index.html or a main.js template string (renamed/typo'd wiring?): {:?}",
        missing
    );
}

/// The Guard Desk ID-scan feature specifically depends on these elements being
/// present. Pin them so a future markup refactor can't quietly drop the
/// ID-scanning UI wiring.
#[test]
fn guard_desk_id_scan_elements_exist() {
    let html = read("../src/index.html");
    let authored = authored_ids(&html);
    for id in [
        "idName",
        "idDob",
        "idNumber",
        "idExpiry",
        "idIssue",
        "idSex",
        "idClass",
        "idJurisdiction",
        "idDocType",
        "guardIdFlags",
        "guardRuleList",
        "guardScanBtn",
        "guardIdImageInput",
    ] {
        assert!(
            authored.contains(id),
            "expected Guard Desk element id `{id}` in src/index.html",
        );
    }
}
