//! macOS Accessibility semantic UI operations via optional GhostAXHelper.

use crate::runtime::evidence::StepEvidence;
use crate::runtime::helper_budget::{self, HelperBudget};
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
    /// Opt-in PNG crop for template-after-OCR fallback (`core/template_match`).
    /// Absent by default — never ambient capture; only used when the plan
    /// explicitly carries a fragment (same opt-in spirit as
    /// `PerformanceSettings::capture_element_templates` for replay).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_png: Option<Box<[u8]>>,
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
            template_png: None,
        }
    }

    pub fn with_template_png(mut self, png: impl Into<Box<[u8]>>) -> Self {
        self.template_png = Some(png.into());
        self
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
        self.to_ax_request_with_budget(op, None)
    }

    fn to_ax_request_with_budget(&self, op: &str, budget: Option<HelperBudget>) -> AxRequest {
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
            budget_ms: budget.map(|b| b.budget_ms),
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
    /// Cooperative mid-op budget exhausted inside the helper (or before spawn).
    TimedOut(String),
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
            Self::TimedOut(d) => write!(f, "semantic timed out: {d}"),
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
    /// Wall-clock ms from helper request start; cooperative mid-op deadline.
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_ms: Option<u32>,
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

fn ensure_budget(budget: Option<HelperBudget>) -> Result<(), SemanticError> {
    if let Some(b) = budget
        && b.is_exhausted()
    {
        return Err(SemanticError::TimedOut(
            helper_budget::HELPER_BUDGET_EXCEEDED.into(),
        ));
    }
    Ok(())
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn map_helper_response(resp: AxResponse) -> Result<AxResponse, SemanticError> {
    if !resp.ok && helper_budget::is_helper_budget_timeout(&resp.detail) {
        return Err(SemanticError::TimedOut(resp.detail));
    }
    if !resp.ok && resp.detail.contains("accessibility denied") {
        return Err(SemanticError::PermissionDenied(resp.detail));
    }
    Ok(resp)
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
        map_helper_response(resp)
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
        budget_ms: None,
    })?;
    Ok(resp.ok)
}

pub fn resolve_target(target: &UiTarget) -> Result<ResolvedTarget, SemanticError> {
    resolve_target_with_budget(target, None)
}

pub fn resolve_target_with_budget(
    target: &UiTarget,
    budget: Option<HelperBudget>,
) -> Result<ResolvedTarget, SemanticError> {
    ensure_budget(budget)?;
    let resp = call_helper(&target.to_ax_request_with_budget("resolve_target", budget))?;
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

fn activate_resolved(
    target: &UiTarget,
    resolved: &ResolvedTarget,
    budget: Option<HelperBudget>,
) -> Result<(), SemanticError> {
    ensure_fresh(target, resolved)?;
    ensure_budget(budget)?;
    let mut req = target.to_ax_request_with_budget("activate_element", budget);
    req.fingerprint = Some(resolved.fingerprint.clone());
    let resp = call_helper(&req)?;
    if resp.ok {
        Ok(())
    } else if helper_budget::is_helper_budget_timeout(&resp.detail) {
        Err(SemanticError::TimedOut(resp.detail))
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
        VisionFallbackError::Capture(CaptureError::TimedOut(d)) => SemanticError::TimedOut(d),
        other => SemanticError::Failed(other.to_string()),
    }
}

fn try_vision_focus(
    target: &UiTarget,
    budget: Option<HelperBudget>,
) -> Result<StepEvidence, SemanticError> {
    // Order: OCR (needs title) → template (needs opt-in template_png) → fail.
    let mut last = None;
    if let Some(needle) = search_text(target) {
        match crate::runtime::vision_fallback::focus_via_ocr_with_budget(
            needle,
            target.bundle_id.as_deref(),
            target.window_title.as_deref(),
            budget,
        ) {
            Ok(hit) => {
                let mut ev = StepEvidence::ocr(hit.fingerprint, hit.text);
                if let Some(path) = hit.capture_path {
                    ev = ev.with_capture_path(path);
                }
                return Ok(ev);
            }
            Err(e) => last = Some(e),
        }
    }
    if let Some(png) = target.template_png.as_deref() {
        let hit = crate::runtime::vision_fallback::focus_via_template_with_budget(png, budget)
            .map_err(map_vision_err)?;
        let mut ev = StepEvidence::template(hit.fingerprint, hit.text);
        if let Some(path) = hit.capture_path {
            ev = ev.with_capture_path(path);
        }
        return Ok(ev);
    }
    Err(map_vision_err(last.unwrap_or(
        crate::runtime::vision_fallback::VisionFallbackError::NoSearchText,
    )))
}

fn try_vision_set_value(
    target: &UiTarget,
    value: &str,
    budget: Option<HelperBudget>,
) -> Result<StepEvidence, SemanticError> {
    let mut last = None;
    if let Some(needle) = search_text(target) {
        match crate::runtime::vision_fallback::set_value_via_ocr_with_budget(
            needle,
            value,
            target.bundle_id.as_deref(),
            target.window_title.as_deref(),
            budget,
        ) {
            Ok(hit) => {
                let mut ev = StepEvidence::ocr(hit.fingerprint, hit.text);
                if let Some(path) = hit.capture_path {
                    ev = ev.with_capture_path(path);
                }
                return Ok(ev);
            }
            Err(e) => last = Some(e),
        }
    }
    if let Some(png) = target.template_png.as_deref() {
        let hit =
            crate::runtime::vision_fallback::set_value_via_template_with_budget(png, value, budget)
                .map_err(map_vision_err)?;
        let mut ev = StepEvidence::template(hit.fingerprint, hit.text);
        if let Some(path) = hit.capture_path {
            ev = ev.with_capture_path(path);
        }
        return Ok(ev);
    }
    Err(map_vision_err(last.unwrap_or(
        crate::runtime::vision_fallback::VisionFallbackError::NoSearchText,
    )))
}

/// Precondition: target application must be running (NSWorkspace; no AX grant required).
///
/// When the helper is unavailable (Linux CI / missing sidecar), the check is
/// skipped so AX/vision paths can report their own errors.
pub fn ensure_app_running(target: &UiTarget) -> Result<(), SemanticError> {
    match call_helper(&AxRequest {
        op: "app_running".into(),
        app: Some(target.app.clone()),
        role: None,
        title: None,
        value: None,
        fingerprint: None,
        expected_value: None,
        identifier: None,
        bundle_id: target.bundle_id.clone(),
        window_title: None,
        budget_ms: None,
    }) {
        Ok(resp) if resp.ok => Ok(()),
        Ok(resp) => Err(SemanticError::NotFound(resp.detail)),
        Err(SemanticError::HelperUnavailable(_)) => Ok(()),
        Err(e) => Err(e),
    }
}

fn with_ax_then_vision<F>(
    target: &UiTarget,
    budget: Option<HelperBudget>,
    mut ax_action: F,
    vision_action: impl FnOnce() -> Result<StepEvidence, SemanticError>,
) -> Result<StepEvidence, SemanticError>
where
    F: FnMut(&ResolvedTarget) -> Result<StepEvidence, SemanticError>,
{
    ensure_app_running(target)?;
    let has_text = search_text(target).is_some();
    let has_template = target.template_png.as_ref().is_some_and(|p| !p.is_empty());
    match resolve_target_with_budget(target, budget) {
        Ok(resolved) => {
            if !crate::runtime::vision_fallback::should_prefer_vision_after_resolve(
                resolved.quality,
                has_text,
                has_template,
            ) {
                return ax_action(&resolved);
            }
            match ax_action(&resolved) {
                Ok(evidence) => Ok(evidence),
                Err(ax_err)
                    if crate::runtime::vision_fallback::should_attempt_vision_for_error(
                        vision_failure_kind(&ax_err),
                        has_text,
                        has_template,
                    ) =>
                {
                    vision_action()
                }
                Err(e) => Err(e),
            }
        }
        Err(ax_err)
            if crate::runtime::vision_fallback::should_attempt_vision_for_error(
                vision_failure_kind(&ax_err),
                has_text,
                has_template,
            ) =>
        {
            vision_action()
        }
        Err(e) => Err(e),
    }
}

/// Focus a target; returns compact resolution evidence (no screenshot retention).
pub fn focus_target(target: &UiTarget) -> Result<StepEvidence, SemanticError> {
    focus_target_with_budget(target, None)
}

/// Like [`focus_target`], passing a cooperative mid-op helper budget.
pub fn focus_target_with_budget(
    target: &UiTarget,
    budget: Option<HelperBudget>,
) -> Result<StepEvidence, SemanticError> {
    with_ax_then_vision(
        target,
        budget,
        |resolved| {
            activate_resolved(target, resolved, budget)?;
            Ok(StepEvidence::ax(
                resolved.quality.score,
                resolved.fingerprint.clone(),
            ))
        },
        || try_vision_focus(target, budget),
    )
}

/// Set a value on a target; returns compact resolution evidence (no screenshot retention).
pub fn set_target_value(target: &UiTarget, value: &str) -> Result<StepEvidence, SemanticError> {
    set_target_value_with_budget(target, value, None)
}

/// Like [`set_target_value`], passing a cooperative mid-op helper budget.
pub fn set_target_value_with_budget(
    target: &UiTarget,
    value: &str,
    budget: Option<HelperBudget>,
) -> Result<StepEvidence, SemanticError> {
    with_ax_then_vision(
        target,
        budget,
        |resolved| {
            ensure_fresh(target, resolved)?;
            ensure_budget(budget)?;
            let mut req = target.to_ax_request_with_budget("set_value", budget);
            req.value = Some(value.into());
            req.fingerprint = Some(resolved.fingerprint.clone());
            let resp = call_helper(&req)?;
            if resp.ok {
                Ok(StepEvidence::ax(
                    resolved.quality.score,
                    resolved.fingerprint.clone(),
                ))
            } else if helper_budget::is_helper_budget_timeout(&resp.detail) {
                Err(SemanticError::TimedOut(resp.detail))
            } else {
                Err(SemanticError::Failed(resp.detail))
            }
        },
        || try_vision_set_value(target, value, budget),
    )
}

/// Best-effort postcondition: AX verify, else OCR presence of `expected` / title.
pub fn verify_postcondition(
    target: &UiTarget,
    expected_value: Option<&str>,
) -> Result<String, SemanticError> {
    match verify_target(target, expected_value) {
        Ok(observed) => Ok(observed),
        Err(SemanticError::HelperUnavailable(msg)) => Err(SemanticError::HelperUnavailable(msg)),
        Err(ax_err) => {
            let needle = expected_value
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .or_else(|| search_text(target));
            let Some(needle) = needle else {
                return Err(ax_err);
            };
            let hit = crate::runtime::vision_fallback::resolve_text(
                needle,
                target.bundle_id.as_deref(),
                target.window_title.as_deref(),
                true,
            )
            .map_err(map_vision_err)?;
            Ok(format!("ocr:{}", hit.text))
        }
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
        let timed = SemanticError::TimedOut(helper_budget::HELPER_BUDGET_EXCEEDED.into());
        assert!(timed.to_string().contains("timed out"));
    }

    #[test]
    fn ax_request_serializes_budget_ms() {
        let target = UiTarget::new("TextEdit", "AXTextArea");
        let req = target.to_ax_request_with_budget("resolve_target", Some(HelperBudget::new(900)));
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("budget_ms"));
        assert!(json.contains("900"));
    }

    #[test]
    fn exhausted_budget_fails_before_helper() {
        let target = UiTarget::new("TextEdit", "AXTextArea");
        let err = resolve_target_with_budget(&target, Some(HelperBudget::new(0))).unwrap_err();
        assert!(matches!(err, SemanticError::TimedOut(_)));
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
