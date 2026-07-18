//! ScreenCaptureKit still-frame capture via GhostAXHelper.
//!
//! Capture is a separate permission surface from Accessibility. When Screen
//! Recording is denied, AX-only automation must keep working.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use std::io::{BufRead, BufReader, Write};
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    PermissionDenied(String),
    HelperUnavailable(String),
    Failed(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermissionDenied(d) => write!(f, "screen recording denied: {d}"),
            Self::HelperUnavailable(d) => write!(f, "capture helper unavailable: {d}"),
            Self::Failed(d) => write!(f, "{d}"),
        }
    }
}

impl std::error::Error for CaptureError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaptureRequest {
    op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaptureResponse {
    ok: bool,
    detail: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StillFrame {
    pub path: PathBuf,
    pub width: Option<u32>,
    pub height: Option<u32>,
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

fn call_helper(req: &CaptureRequest) -> Result<CaptureResponse, CaptureError> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = req;
        Err(CaptureError::HelperUnavailable(
            "ScreenCaptureKit helper is macOS-only".into(),
        ))
    }

    #[cfg(target_os = "macos")]
    {
        let path = helper_path().ok_or_else(|| {
            CaptureError::HelperUnavailable(
                "GhostAXHelper not found — rebuild on macOS or set GHOST_AX_HELPER".into(),
            )
        })?;
        let mut child = Command::new(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::HelperUnavailable(e.to_string()))?;

        let payload =
            serde_json::to_string(req).map_err(|e| CaptureError::Failed(e.to_string()))?;
        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| CaptureError::Failed("capture helper stdin unavailable".into()))?;
            writeln!(stdin, "{payload}").map_err(|e| CaptureError::Failed(e.to_string()))?;
        }

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CaptureError::Failed("capture helper stdout unavailable".into()))?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| CaptureError::Failed(e.to_string()))?;
        let _ = child.wait();

        let resp: CaptureResponse =
            serde_json::from_str(line.trim()).map_err(|e| CaptureError::Failed(e.to_string()))?;
        if !resp.ok
            && (resp.detail.contains("screen recording denied")
                || resp.detail.contains("ScreenCaptureKit")
                    && resp.detail.to_lowercase().contains("denied"))
        {
            return Err(CaptureError::PermissionDenied(resp.detail));
        }
        Ok(resp)
    }
}

/// Probe Screen Recording without prompting.
pub fn capture_permission_granted() -> Result<bool, CaptureError> {
    let resp = call_helper(&CaptureRequest {
        op: "capture_permission_status".into(),
        path: None,
        bundle_id: None,
        window_title: None,
    })?;
    Ok(resp.ok)
}

/// Capture a still frame via ScreenCaptureKit into `dest_png`.
///
/// When `bundle_id` / `window_title` are set, the helper prefers that window;
/// otherwise it captures the primary display.
pub fn capture_still(
    dest_png: &Path,
    bundle_id: Option<&str>,
    window_title: Option<&str>,
) -> Result<StillFrame, CaptureError> {
    let resp = call_helper(&CaptureRequest {
        op: "capture_still".into(),
        path: Some(dest_png.to_string_lossy().into_owned()),
        bundle_id: bundle_id.map(str::to_string),
        window_title: window_title.map(str::to_string),
    })?;
    if !resp.ok {
        return Err(CaptureError::Failed(resp.detail));
    }
    let path = resp
        .path
        .map(PathBuf::from)
        .unwrap_or_else(|| dest_png.to_path_buf());
    if !path.is_file() {
        return Err(CaptureError::Failed(format!(
            "capture_still reported ok but file missing: {}",
            path.display()
        )));
    }
    Ok(StillFrame {
        path,
        width: resp.width,
        height: resp.height,
    })
}

/// Prefer ScreenCaptureKit helper; fall back to legacy `screencapture` / platform path.
pub fn capture_still_bytes(
    bundle_id: Option<&str>,
    window_title: Option<&str>,
) -> Result<Vec<u8>, CaptureError> {
    let dest = std::env::temp_dir().join(format!("ghost_sck_{}.png", uuid::Uuid::new_v4()));
    match capture_still(&dest, bundle_id, window_title) {
        Ok(frame) => {
            let bytes =
                std::fs::read(&frame.path).map_err(|e| CaptureError::Failed(e.to_string()))?;
            let _ = std::fs::remove_file(&frame.path);
            Ok(bytes)
        }
        Err(CaptureError::HelperUnavailable(_)) | Err(CaptureError::PermissionDenied(_)) => {
            crate::core::vision::capture_screenshot()
                .map_err(|e| CaptureError::Failed(e.to_string()))
        }
        Err(e) => {
            // Helper present but capture failed — try legacy path once.
            match crate::core::vision::capture_screenshot() {
                Ok(bytes) => Ok(bytes),
                Err(_) => Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_errors_display_usefully() {
        let err = CaptureError::PermissionDenied("screen recording denied".into());
        assert!(err.to_string().contains("screen recording"));
    }

    #[test]
    fn capture_helper_unavailable_off_macos() {
        let dest = std::env::temp_dir().join("ghost_capture_test_missing.png");
        let err = capture_still(&dest, None, None).unwrap_err();
        match err {
            CaptureError::HelperUnavailable(_) | CaptureError::Failed(_) => {}
            other => panic!("unexpected error: {other}"),
        }
    }
}
