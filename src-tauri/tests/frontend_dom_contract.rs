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

/// Every `bind("id", …)` in wireUpControls must point at a real element.
fn bind_targets(js: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"bind\(\s*["']([A-Za-z0-9_-]+)["']\s*,"#).unwrap();
    re.captures_iter(js).map(|c| c[1].to_string()).collect()
}

#[test]
fn wireup_bind_targets_are_authored() {
    let js = read("../src/main.js");
    let html = read("../src/index.html");

    let mut authored = authored_ids(&html);
    authored.extend(authored_ids(&js));

    let referenced = bind_targets(&js);
    assert!(
        referenced.len() > 40,
        "bind() parsing looks broken — only found {} references",
        referenced.len()
    );

    let missing: Vec<String> = referenced
        .into_iter()
        .filter(|id| !authored.contains(id))
        .collect();

    assert!(
        missing.is_empty(),
        "wireUpControls bind() targets ids that are never authored in \
         src/index.html or a main.js template string: {:?}",
        missing
    );
}

fn function_body<'a>(js: &'a str, name: &str) -> &'a str {
    let start = js
        .find(&format!("function {name}("))
        .unwrap_or_else(|| panic!("expected function {name} in src/main.js"));
    let open = js[start..]
        .find('{')
        .map(|idx| start + idx)
        .unwrap_or_else(|| panic!("expected opening brace for function {name}"));

    let mut depth = 0i32;
    for (idx, ch) in js[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &js[start..=open + idx];
                }
            }
            _ => {}
        }
    }

    panic!("expected closing brace for function {name}");
}

#[test]
fn module_initializers_wire_static_feature_controls() {
    let js = read("../src/main.js");

    let guard_init = function_body(&js, "guardDeskInit");
    for id in [
        "guardScanBtn",
        "guardUploadCheckBtn",
        "guardUploadIdBtn",
        "guardAutofillBtn",
        "guardResetBtn",
    ] {
        assert!(
            guard_init.contains(&format!("\"{id}\"")) && guard_init.contains("addEventListener"),
            "guardDeskInit must wire static Guard Desk control `{id}`"
        );
    }

    // Approve is enabled after scan results, not at init time — keep that
    // trust-pipeline order (recommend → human approve → POS Bridge).
    let show_results = function_body(&js, "guardShowComplianceResults");
    assert!(
        show_results.contains("\"guardApproveBtn\"") && show_results.contains("approveBtn.onclick"),
        "Guard Desk approve control must be enabled and wired after a scan result"
    );

    let filing_init = function_body(&js, "filingInit");
    for id in [
        "filingPreviewBtn",
        "filingSampleBtn",
        "filingClearBtn",
        "savingsEstimateBtn",
    ] {
        assert!(
            filing_init.contains(&format!("\"{id}\"")) && filing_init.contains("addEventListener"),
            "filingInit must wire static Plan Filing control `{id}`"
        );
    }

    let dom_ready = js
        .find("window.addEventListener(\"DOMContentLoaded\"")
        .map(|idx| &js[idx..])
        .expect("expected DOMContentLoaded bootstrap in src/main.js");
    for initializer in [
        "guardDeskInit();",
        "filingInit();",
        "organizerInit();",
        "wireUpControls();",
    ] {
        assert!(
            dom_ready.contains(initializer),
            "DOMContentLoaded bootstrap must call `{initializer}`"
        );
    }
}

fn data_view_values(html: &str, attr_prefix: &str) -> HashSet<String> {
    let re =
        regex::Regex::new(&format!(r#"{attr_prefix}\s*=\s*["']([A-Za-z0-9_-]+)["']"#)).unwrap();
    re.captures_iter(html).map(|c| c[1].to_string()).collect()
}

#[test]
fn nav_data_view_values_have_matching_panels() {
    let html = read("../src/index.html");
    let nav_views = data_view_values(&html, "data-view");
    assert!(
        nav_views.len() >= 5,
        "data-view parsing looks broken — only found {} nav views",
        nav_views.len()
    );

    // Every nav tab must have a matching `.view` section (and vice versa).
    let panel_re =
        regex::Regex::new(r#"class="view[^"]*"[^>]*data-view\s*=\s*["']([A-Za-z0-9_-]+)["']"#)
            .unwrap();
    let panel_views: HashSet<String> = panel_re
        .captures_iter(&html)
        .map(|c| c[1].to_string())
        .collect();

    let nav_only: Vec<String> = nav_views.difference(&panel_views).cloned().collect();
    let panel_only: Vec<String> = panel_views.difference(&nav_views).cloned().collect();

    assert!(
        nav_only.is_empty() && panel_only.is_empty(),
        "nav/panel data-view mismatch — nav-only: {:?}, panel-only: {:?}",
        nav_only,
        panel_only
    );
}

#[test]
fn compression_review_container_ids_exist() {
    let html = read("../src/index.html");
    let authored = authored_ids(&html);
    for id in ["events-timeline", "review-modal-compression"] {
        assert!(
            authored.contains(id),
            "CompressionReview container id `{id}` must exist in src/index.html",
        );
    }
}

#[test]
fn open_settings_authors_account_sign_in_controls() {
    let js = read("../src/main.js");
    let body = function_body(&js, "openSettings");
    for marker in [
        "data-account-sign-in=\"google\"",
        "data-account-sign-in=\"microsoft\"",
        "google_sign_in_available",
        "microsoft_sign_in_available",
        "id=\"account-status-note\"",
        "data-account-sign-out",
    ] {
        assert!(
            body.contains(marker),
            "openSettings must render account control `{marker}`"
        );
    }
}

#[test]
fn replay_unfinished_banner_exists() {
    let html = read("../src/index.html");
    let authored = authored_ids(&html);
    assert!(
        authored.contains("replayUnfinishedBanner"),
        "record view must include replayUnfinishedBanner for crash recovery"
    );
}

#[test]
fn mission_steps_include_policy_gate() {
    let html = read("../src/index.html");
    assert!(
        html.contains("data-mission-step=\"policy\""),
        "mission steps must include a policy review step"
    );
}

#[test]
fn review_modal_authors_policy_replay_actions() {
    let js = read("../src/main.js");
    let body = function_body(&js, "renderReviewModalActions");
    for marker in [
        "review-modal-actions",
        "review-modal-approve-replay",
        "Approve & Replay",
        "can_proceed_with_approvals",
    ] {
        assert!(
            body.contains(marker),
            "renderReviewModalActions must wire policy replay controls `{marker}`"
        );
    }
}

#[test]
fn wireup_delegates_account_sign_in_and_out() {
    let js = read("../src/main.js");
    let body = function_body(&js, "wireUpControls");
    assert!(
        body.contains("[data-account-sign-in]") && body.contains("signInWithProvider"),
        "wireUpControls must delegate account sign-in clicks"
    );
    assert!(
        body.contains("[data-account-sign-out]") && body.contains("signOutAccount"),
        "wireUpControls must delegate account sign-out clicks"
    );
}

/// Pins the production Content-Security-Policy. `script-src 'self'` is the
/// XSS-relevant control for a Tauri webview (it is what keeps injected HTML
/// from reaching the IPC bridge); this test keeps it from quietly regaining
/// `'unsafe-inline'` or new sources. `style-src` intentionally still allows
/// inline styles (~200 style attributes in the templates) — see
/// `.github/ISSUES/072-csp-nonce.md` for the tracked migration.
#[test]
fn production_csp_stays_locked_down() {
    let conf: serde_json::Value =
        serde_json::from_str(&read("tauri.conf.json")).expect("tauri.conf.json parses");
    let csp = conf["app"]["security"]["csp"]
        .as_str()
        .expect("app.security.csp is a string");

    let directive = |name: &str| -> Option<String> {
        csp.split(';')
            .map(str::trim)
            .find(|d| d.starts_with(name) && d[name.len()..].starts_with(' '))
            .map(|d| d[name.len()..].trim().to_string())
    };

    assert_eq!(
        directive("script-src").as_deref(),
        Some("'self'"),
        "script-src must be exactly 'self' — no inline scripts, no extra sources"
    );
    assert_eq!(
        directive("default-src").as_deref(),
        Some("'self'"),
        "default-src must be exactly 'self'"
    );
    assert_eq!(
        directive("object-src").as_deref(),
        Some("'none'"),
        "object-src must be 'none'"
    );
    assert_eq!(
        directive("frame-src").as_deref(),
        Some("'none'"),
        "frame-src must be 'none'"
    );
}

/// The CSP above only holds if the page itself carries no inline scripts:
/// every `<script>` in index.html must load an external module (Vite keeps the
/// bundle external-only; see vite.config.js).
#[test]
fn index_html_has_no_inline_scripts() {
    let html = read("../src/index.html");
    for (idx, _) in html.match_indices("<script") {
        let tag_end = html[idx..].find('>').map(|e| idx + e).unwrap_or(html.len());
        let tag = &html[idx..tag_end];
        assert!(
            tag.contains("src="),
            "inline <script> without src= found in src/index.html: `{tag}`"
        );
    }
}
