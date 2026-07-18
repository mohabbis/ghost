//! macOS Accessibility semantic UI operations via optional GhostAXHelper.

use crate::runtime::locator::{self, AxQuality, Locator};

use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use std::io::{BufRead, BufReader, Write};
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};

/// Semantic UI target resolved through Accessibility (and later vision fallbacks).
///
/// New optional fields are serde-defaulted so older plans keep loading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTarget {
    pub app: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Captured at plan time; execution refuses when the live target drifts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// Stable AXIdentifier when the app exposes one (strongest match signal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    /// Application bundle identifier when known (preferred over fuzzy app name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    /// Containing window title used to narrow the AX search root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
}

impl UiTarget {
    pub fn new(app: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            role: role.into(),
            title: None,
            fingerprint: None,
            identifier: None,
            bundle_id: None,
            window_title: None,
        }
    }

    /// Project this target into the shared [`Locator`] format.
    pub fn to_locator(&self) -> Locator {
        Locator::Accessibility {
            role: Some(self.role.clone()),
            title: self.title.clone(),
            identifier: self.identifier.clone(),
            value: None,
            ancestor_path: self
                .window_title
                .as_ref()
                .map(|title| {
                    vec![locator::AxConstraint {
                        role: Some("AXWindow".into()),
                        title: Some(title.clone()),
                        identifier: None,
                    }]
                })
                .unwrap_or_default(),
        }
    }

    fn to_ax_request(&self, op: &str) -> AxRequest {
        AxRequest {
            op: op.into(),
            app: Some(self.app.clone()),
            role: Some(self.role.clone()),
            title: self.title.clone(),
            value: None,
            fingerprint: self.fingerprint.clone(),
            expected_value: None,
            identifier: self.identifier.clone(),
            bundle_id: self.bundle_id.clone(),
            window_title: self.window_title.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticError {
    NotFound(String),
    Ambiguous(usize),
    StaleTarget {
        expected: String,
        observed: String,
    },
    PermissionDenied(String),
    HelperUnavailable(String),
    /// AX tree present but scored too weak for a trusted action.
    /// Reserved for when vision fallback is wired; resolve currently reports
    /// quality on [`ResolvedTarget`] instead of hard-failing.
    InsufficientAx(AxQuality),
    Failed(String),
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(d) => write!(f, "semantic target not found: {d}"),
            Self::Ambiguous(n) => write!(f, "ambiguous semantic target ({n} matches)"),
            Self::StaleTarget { expected, observed } => {
                write!(f, "stale target (expected {expected}, observed {observed})")
            }
            Self::PermissionDenied(d) => write!(f, "accessibility denied: {d}"),
            Self::HelperUnavailable(d) => write!(f, "AX helper unavailable: {d}"),
            Self::InsufficientAx(q) => write!(
                f,
                "insufficient accessibility hierarchy (score {}, unique={}, actionable={})",
                q.score, q.unique, q.actionable
            ),
            Self::Failed(d) => write!(f, "{d}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AxRequest {
    op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AxResponse {
    ok: bool,
    detail: String,
    #[serde(default)]
    match_count: Option<u32>,
    #[serde(default)]
    fingerprint: Option<String>,
    #[serde(default)]
    value: Option<String>,
    /// 0–100 AX quality score from the helper when available.
    #[serde(default)]
    ax_quality: Option<u8>,
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    actionable: Option<bool>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTarget {
    pub fingerprint: String,
    pub detail: String,
    pub quality: AxQuality,
}

#[cfg(target_os = "macos")]
fn helper_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(path) = std::env::var("GHOST_AX_HELPER") {
        out.push(PathBuf::from(path));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        // Tauri externalBin sidecar: Ghost.app/Contents/MacOS/ghost-ax-helper
        out.push(dir.join("ghost-ax-helper"));
        if dir.ends_with("MacOS")
            && let Some(contents) = dir.parent()
        {
            out.push(contents.join("Resources").join("ghost-ax-helper"));
        }
    }
    out.extend([
        PathBuf::from("native/macos/ghost-ax-helper"),
        PathBuf::from("../native/macos/ghost-ax-helper"),
        PathBuf::from("src-tauri/bin/ghost-ax-helper-aarch64-apple-darwin"),
        PathBuf::from("src-tauri/bin/ghost-ax-helper-x86_64-apple-darwin"),
        PathBuf::from("bin/ghost-ax-helper-aarch64-apple-darwin"),
        PathBuf::from("bin/ghost-ax-helper-x86_64-apple-darwin"),
    ]);
    out
}

#[cfg(target_os = "macos")]
fn helper_path() -> Option<PathBuf> {
    helper_candidates().into_iter().find(|c| c.is_file())
}

fn call_helper(req: &AxRequest) -> Result<AxResponse, SemanticError> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = req;
        Err(SemanticError::HelperUnavailable(
            "semantic AX helper is macOS-only".into(),
        ))
    }

    #[cfg(target_os = "macos")]
    {
        let path = helper_path().ok_or_else(|| {
            SemanticError::HelperUnavailable(
                "GhostAXHelper not found — rebuild on macOS or set GHOST_AX_HELPER".into(),
            )
        })?;
        let mut child = Command::new(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| SemanticError::HelperUnavailable(e.to_string()))?;

        let payload =
            serde_json::to_string(req).map_err(|e| SemanticError::Failed(e.to_string()))?;
        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| SemanticError::Failed("AX helper stdin unavailable".into()))?;
            writeln!(stdin, "{payload}").map_err(|e| SemanticError::Failed(e.to_string()))?;
        }

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SemanticError::Failed("AX helper stdout unavailable".into()))?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| SemanticError::Failed(e.to_string()))?;
        let _ = child.wait();

        let resp: AxResponse =
            serde_json::from_str(line.trim()).map_err(|e| SemanticError::Failed(e.to_string()))?;
        if !resp.ok && resp.detail.contains("accessibility denied") {
            return Err(SemanticError::PermissionDenied(resp.detail));
        }
        Ok(resp)
    }
}

fn quality_from_response(resp: &AxResponse, target: &UiTarget) -> AxQuality {
    let match_count = resp.match_count.unwrap_or(if resp.ok { 1 } else { 0 }) as usize;
    let actionable = resp.actionable.unwrap_or(true);
    if let Some(score) = resp.ax_quality {
        return AxQuality {
            score,
            actionable,
            unique: match_count == 1,
            has_identifier: resp
                .identifier
                .as_deref()
                .or(target.identifier.as_deref())
                .is_some_and(|i| !i.is_empty()),
            has_role: !target.role.is_empty(),
            has_title: target.title.as_deref().is_some_and(|t| !t.is_empty()),
        };
    }
    locator::score_ax_candidate(
        Some(target.role.as_str()),
        target.title.as_deref(),
        resp.identifier.as_deref().or(target.identifier.as_deref()),
        actionable,
        match_count,
    )
}

pub fn permission_status() -> Result<bool, SemanticError> {
    let resp = call_helper(&AxRequest {
        op: "permission_status".into(),
        app: None,
        role: None,
        title: None,
        value: None,
        fingerprint: None,
        expected_value: None,
        identifier: None,
        bundle_id: None,
        window_title: None,
    })?;
    Ok(resp.ok)
}

pub fn resolve_target(target: &UiTarget) -> Result<ResolvedTarget, SemanticError> {
    let resp = call_helper(&target.to_ax_request("resolve_target"))?;
    let quality = quality_from_response(&resp, target);

    if let Some(count) = resp.match_count {
        if count == 0 {
            return Err(SemanticError::NotFound(resp.detail));
        }
        if count > 1 {
            return Err(SemanticError::Ambiguous(count as usize));
        }
    }
    if !resp.ok {
        if resp.detail.contains("ambiguous") {
            let count = resp.match_count.unwrap_or(2) as usize;
            return Err(SemanticError::Ambiguous(count));
        }
        // Helper may refuse a uniquely matched but unscorable tree. Surface the
        // scored quality so callers can choose vision later; do not soft-succeed.
        if resp.detail.contains("insufficient") {
            return Err(SemanticError::InsufficientAx(quality));
        }
        return Err(SemanticError::NotFound(resp.detail));
    }
    let fingerprint = resp
        .fingerprint
        .ok_or_else(|| SemanticError::Failed("resolve_target missing fingerprint".into()))?;
    Ok(ResolvedTarget {
        fingerprint,
        detail: resp.detail,
        quality,
    })
}

fn ensure_fresh(target: &UiTarget, resolved: &ResolvedTarget) -> Result<(), SemanticError> {
    if let Some(expected) = &target.fingerprint
        && expected != &resolved.fingerprint
    {
        return Err(SemanticError::StaleTarget {
            expected: expected.clone(),
            observed: resolved.fingerprint.clone(),
        });
    }
    Ok(())
}

fn activate_resolved(target: &UiTarget, resolved: &ResolvedTarget) -> Result<(), SemanticError> {
    ensure_fresh(target, resolved)?;
    let mut req = target.to_ax_request("activate_element");
    req.fingerprint = Some(resolved.fingerprint.clone());
    let resp = call_helper(&req)?;
    if resp.ok {
        Ok(())
    } else {
        Err(SemanticError::Failed(resp.detail))
    }
}

fn search_text(target: &UiTarget) -> Option<&str> {
    target
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

fn vision_failure_kind(err: &SemanticError) -> crate::runtime::vision_fallback::VisionAxFailure {
    use crate::runtime::vision_fallback::VisionAxFailure;
    match err {
        SemanticError::NotFound(_) => VisionAxFailure::NotFound,
        SemanticError::InsufficientAx(_) => VisionAxFailure::InsufficientAx,
        SemanticError::Ambiguous(_) => VisionAxFailure::Ambiguous,
        SemanticError::HelperUnavailable(_) => VisionAxFailure::HelperUnavailable,
        SemanticError::Failed(d) if d.contains("no matching") => VisionAxFailure::NotFound,
        _ => VisionAxFailure::Other,
    }
}

fn map_vision_err(err: crate::runtime::vision_fallback::VisionFallbackError) -> SemanticError {
    use crate::runtime::capture::CaptureError;
    use crate::runtime::vision_fallback::VisionFallbackError;
    match err {
        VisionFallbackError::Ambiguous(n) => SemanticError::Ambiguous(n),
        VisionFallbackError::Capture(CaptureError::PermissionDenied(d)) => {
            SemanticError::PermissionDenied(d)
        }
        other => SemanticError::Failed(other.to_string()),
    }
}

fn try_vision_focus(target: &UiTarget) -> Result<(), SemanticError> {
    let needle = search_text(target).ok_or_else(|| {
        SemanticError::Failed("vision fallback needs target.title text to search".into())
    })?;
    crate::runtime::vision_fallback::focus_via_ocr(
        needle,
        target.bundle_id.as_deref(),
        target.window_title.as_deref(),
    )
    .map(|_| ())
    .map_err(map_vision_err)
}

pub fn focus_target(target: &UiTarget) -> Result<(), SemanticError> {
    let has_text = search_text(target).is_some();
    match resolve_target(target) {
        Ok(resolved) => {
            // Strong AX hit: activate semantically.
            if !crate::runtime::vision_fallback::should_prefer_vision_after_resolve(
                resolved.quality,
                has_text,
            ) {
                return activate_resolved(target, &resolved);
            }
            // Weak but unique AX tree: try AX first, then OCR click.
            match activate_resolved(target, &resolved) {
                Ok(()) => Ok(()),
                Err(ax_err)
                    if crate::runtime::vision_fallback::should_attempt_vision_for_error(
                        vision_failure_kind(&ax_err),
                        has_text,
                    ) =>
                {
                    try_vision_focus(target)
                }
                Err(e) => Err(e),
            }
        }
        Err(ax_err)
            if crate::runtime::vision_fallback::should_attempt_vision_for_error(
                vision_failure_kind(&ax_err),
                has_text,
            ) =>
        {
            try_vision_focus(target)
        }
        Err(e) => Err(e),
    }
}

pub fn set_target_value(target: &UiTarget, value: &str) -> Result<(), SemanticError> {
    let resolved = resolve_target(target)?;
    ensure_fresh(target, &resolved)?;
    let mut req = target.to_ax_request("set_value");
    req.value = Some(value.into());
    req.fingerprint = Some(resolved.fingerprint);
    let resp = call_helper(&req)?;
    if resp.ok {
        Ok(())
    } else {
        Err(SemanticError::Failed(resp.detail))
    }
}

pub fn verify_target(
    target: &UiTarget,
    expected_value: Option<&str>,
) -> Result<String, SemanticError> {
    let mut req = target.to_ax_request("verify_element");
    req.expected_value = expected_value.map(str::to_string);
    let resp = call_helper(&req)?;
    if resp.ok {
        Ok(resp.value.unwrap_or(resp.detail))
    } else if resp.detail.contains("stale") {
        Err(SemanticError::StaleTarget {
            expected: target.fingerprint.clone().unwrap_or_default(),
            observed: resp.fingerprint.unwrap_or_else(|| resp.detail.clone()),
        })
    } else {
        Err(SemanticError::Failed(resp.detail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_errors_display_usefully() {
        let err = SemanticError::Ambiguous(3);
        assert!(err.to_string().contains("ambiguous"));
        let weak = locator::score_ax_candidate(Some("AXButton"), None, None, false, 2);
        let insuff = SemanticError::InsufficientAx(weak);
        assert!(insuff.to_string().contains("insufficient"));
    }

    #[test]
    fn ui_target_projects_to_accessibility_locator() {
        let mut target = UiTarget::new("TextEdit", "AXTextArea");
        target.window_title = Some("Untitled".into());
        target.identifier = Some("main".into());
        match target.to_locator() {
            Locator::Accessibility {
                role,
                identifier,
                ancestor_path,
                ..
            } => {
                assert_eq!(role.as_deref(), Some("AXTextArea"));
                assert_eq!(identifier.as_deref(), Some("main"));
                assert_eq!(ancestor_path.len(), 1);
            }
            other => panic!("expected accessibility locator, got {other:?}"),
        }
    }

    #[test]
    fn ui_target_json_keeps_legacy_shape() {
        let legacy = r#"{"app":"TextEdit","role":"AXTextArea"}"#;
        let target: UiTarget = serde_json::from_str(legacy).unwrap();
        assert_eq!(target.app, "TextEdit");
        assert!(target.identifier.is_none());
        assert!(target.bundle_id.is_none());
        assert!(target.window_title.is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_candidates_include_dev_and_bundle_paths() {
        let candidates = helper_candidates();
        assert!(candidates.iter().any(|p| p.ends_with("ghost-ax-helper")));
        assert!(
            candidates
                .iter()
                .any(|p| p.ends_with("native/macos/ghost-ax-helper"))
        );
    }

    #[test]
    fn helper_unavailable_off_macos_or_without_binary() {
        let target = UiTarget::new("TextEdit", "AXTextArea");
        let result = resolve_target(&target);
        assert!(result.is_err());
    }
}
