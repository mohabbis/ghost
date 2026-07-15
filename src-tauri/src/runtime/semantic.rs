//! macOS Accessibility semantic UI operations via optional GhostAXHelper.

use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use std::io::{BufRead, BufReader, Write};
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTarget {
    pub app: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Captured at plan time; execution refuses when the live target drifts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticError {
    NotFound(String),
    Ambiguous(usize),
    StaleTarget { expected: String, observed: String },
    PermissionDenied(String),
    HelperUnavailable(String),
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTarget {
    pub fingerprint: String,
    pub detail: String,
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

pub fn permission_status() -> Result<bool, SemanticError> {
    let resp = call_helper(&AxRequest {
        op: "permission_status".into(),
        app: None,
        role: None,
        title: None,
        value: None,
        fingerprint: None,
        expected_value: None,
    })?;
    Ok(resp.ok)
}

pub fn resolve_target(target: &UiTarget) -> Result<ResolvedTarget, SemanticError> {
    let resp = call_helper(&AxRequest {
        op: "resolve_target".into(),
        app: Some(target.app.clone()),
        role: Some(target.role.clone()),
        title: target.title.clone(),
        value: None,
        fingerprint: None,
        expected_value: None,
    })?;

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
        return Err(SemanticError::NotFound(resp.detail));
    }
    let fingerprint = resp
        .fingerprint
        .ok_or_else(|| SemanticError::Failed("resolve_target missing fingerprint".into()))?;
    Ok(ResolvedTarget {
        fingerprint,
        detail: resp.detail,
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

pub fn focus_target(target: &UiTarget) -> Result<(), SemanticError> {
    let resolved = resolve_target(target)?;
    ensure_fresh(target, &resolved)?;
    let resp = call_helper(&AxRequest {
        op: "activate_element".into(),
        app: Some(target.app.clone()),
        role: Some(target.role.clone()),
        title: target.title.clone(),
        value: None,
        fingerprint: Some(resolved.fingerprint),
        expected_value: None,
    })?;
    if resp.ok {
        Ok(())
    } else {
        Err(SemanticError::Failed(resp.detail))
    }
}

pub fn set_target_value(target: &UiTarget, value: &str) -> Result<(), SemanticError> {
    let resolved = resolve_target(target)?;
    ensure_fresh(target, &resolved)?;
    let resp = call_helper(&AxRequest {
        op: "set_value".into(),
        app: Some(target.app.clone()),
        role: Some(target.role.clone()),
        title: target.title.clone(),
        value: Some(value.into()),
        fingerprint: Some(resolved.fingerprint),
        expected_value: None,
    })?;
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
    let resp = call_helper(&AxRequest {
        op: "verify_element".into(),
        app: Some(target.app.clone()),
        role: Some(target.role.clone()),
        title: target.title.clone(),
        value: None,
        fingerprint: target.fingerprint.clone(),
        expected_value: expected_value.map(str::to_string),
    })?;
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
        let target = UiTarget {
            app: "TextEdit".into(),
            role: "AXTextArea".into(),
            title: None,
            fingerprint: None,
        };
        let result = resolve_target(&target);
        assert!(result.is_err());
    }
}
