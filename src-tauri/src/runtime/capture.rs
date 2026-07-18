//! ScreenCaptureKit still-frame and bounded stream capture via GhostAXHelper.
//!
//! Capture is a separate permission surface from Accessibility. When Screen
//! Recording is denied, AX-only automation must keep working.
//!
//! Still-frame (`capture_still`) remains the simple default. Bounded stream
//! (`capture_stream_latest`) is an opt-in reliability upgrade that samples a
//! short SCStream and returns the latest complete frame — never ambient
//! observation.

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

/// Bounds for a short-lived ScreenCaptureKit stream sample.
///
/// Hard caps match GhostAXHelper (`duration_ms` ≤ 2000, `max_frames` ≤ 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamCaptureOpts {
    pub duration_ms: u32,
    pub max_frames: u32,
}

impl Default for StreamCaptureOpts {
    fn default() -> Self {
        Self {
            duration_ms: 400,
            max_frames: 3,
        }
    }
}

impl StreamCaptureOpts {
    pub const MAX_DURATION_MS: u32 = 2000;
    pub const MAX_FRAMES: u32 = 8;
    pub const MIN_DURATION_MS: u32 = 50;

    /// Clamp to helper-enforced limits (also enforced in Swift).
    pub fn clamped(self) -> Self {
        Self {
            duration_ms: self
                .duration_ms
                .clamp(Self::MIN_DURATION_MS, Self::MAX_DURATION_MS),
            max_frames: self.max_frames.clamp(1, Self::MAX_FRAMES),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaptureRequest {
    op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_frames: Option<u32>,
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
    #[serde(default)]
    frames: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StillFrame {
    pub path: PathBuf,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Complete frames observed when the frame came from a bounded stream.
    pub frames: Option<u32>,
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

fn frame_from_response(
    resp: CaptureResponse,
    dest_png: &Path,
    op: &str,
) -> Result<StillFrame, CaptureError> {
    if !resp.ok {
        return Err(CaptureError::Failed(resp.detail));
    }
    let path = resp
        .path
        .map(PathBuf::from)
        .unwrap_or_else(|| dest_png.to_path_buf());
    if !path.is_file() {
        return Err(CaptureError::Failed(format!(
            "{op} reported ok but file missing: {}",
            path.display()
        )));
    }
    Ok(StillFrame {
        path,
        width: resp.width,
        height: resp.height,
        frames: resp.frames,
    })
}

/// Probe Screen Recording without prompting.
pub fn capture_permission_granted() -> Result<bool, CaptureError> {
    let resp = call_helper(&CaptureRequest {
        op: "capture_permission_status".into(),
        path: None,
        bundle_id: None,
        window_title: None,
        duration_ms: None,
        max_frames: None,
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
        duration_ms: None,
        max_frames: None,
    })?;
    frame_from_response(resp, dest_png, "capture_still")
}

/// Bounded ScreenCaptureKit stream sample → latest complete frame.
///
/// Opt-in reliability path for OCR: short-lived, hard-capped, request-scoped.
/// Prefer [`capture_still`] when a single snapshot is enough.
pub fn capture_stream_latest(
    dest_png: &Path,
    bundle_id: Option<&str>,
    window_title: Option<&str>,
    opts: StreamCaptureOpts,
) -> Result<StillFrame, CaptureError> {
    let opts = opts.clamped();
    let resp = call_helper(&CaptureRequest {
        op: "capture_stream_latest".into(),
        path: Some(dest_png.to_string_lossy().into_owned()),
        bundle_id: bundle_id.map(str::to_string),
        window_title: window_title.map(str::to_string),
        duration_ms: Some(opts.duration_ms),
        max_frames: Some(opts.max_frames),
    })?;
    frame_from_response(resp, dest_png, "capture_stream_latest")
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

/// Latest frame for OCR fallback: bounded stream first, then still, then legacy.
///
/// Stream is the reliability upgrade; still-frame remains the fallback when the
/// stream op is unavailable or fails. Never starts ambient capture.
pub fn capture_latest_frame_bytes(
    bundle_id: Option<&str>,
    window_title: Option<&str>,
) -> Result<Vec<u8>, CaptureError> {
    capture_latest_frame_bytes_with_opts(bundle_id, window_title, StreamCaptureOpts::default())
}

/// Same as [`capture_latest_frame_bytes`] with explicit stream bounds.
pub fn capture_latest_frame_bytes_with_opts(
    bundle_id: Option<&str>,
    window_title: Option<&str>,
    opts: StreamCaptureOpts,
) -> Result<Vec<u8>, CaptureError> {
    let dest = std::env::temp_dir().join(format!("ghost_sck_stream_{}.png", uuid::Uuid::new_v4()));
    match capture_stream_latest(&dest, bundle_id, window_title, opts) {
        Ok(frame) => {
            let bytes =
                std::fs::read(&frame.path).map_err(|e| CaptureError::Failed(e.to_string()))?;
            let _ = std::fs::remove_file(&frame.path);
            Ok(bytes)
        }
        Err(CaptureError::HelperUnavailable(_)) | Err(CaptureError::PermissionDenied(_)) => {
            // No stream path — degrade to still / legacy without inventing capture.
            capture_still_bytes(bundle_id, window_title)
        }
        Err(_) => {
            // Stream helper present but sample failed — still-frame once, then legacy.
            match capture_still_bytes(bundle_id, window_title) {
                Ok(bytes) => Ok(bytes),
                Err(still_err) => Err(still_err),
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

    #[test]
    fn stream_opts_clamp_to_helper_limits() {
        let opts = StreamCaptureOpts {
            duration_ms: 50_000,
            max_frames: 99,
        }
        .clamped();
        assert_eq!(opts.duration_ms, StreamCaptureOpts::MAX_DURATION_MS);
        assert_eq!(opts.max_frames, StreamCaptureOpts::MAX_FRAMES);

        let low = StreamCaptureOpts {
            duration_ms: 1,
            max_frames: 0,
        }
        .clamped();
        assert_eq!(low.duration_ms, StreamCaptureOpts::MIN_DURATION_MS);
        assert_eq!(low.max_frames, 1);
    }

    #[test]
    fn stream_latest_unavailable_off_macos() {
        let dest = std::env::temp_dir().join("ghost_stream_test_missing.png");
        let err =
            capture_stream_latest(&dest, None, None, StreamCaptureOpts::default()).unwrap_err();
        match err {
            CaptureError::HelperUnavailable(_) | CaptureError::Failed(_) => {}
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn stream_request_serializes_bounds() {
        let req = CaptureRequest {
            op: "capture_stream_latest".into(),
            path: Some("/tmp/x.png".into()),
            bundle_id: Some("com.apple.TextEdit".into()),
            window_title: None,
            duration_ms: Some(400),
            max_frames: Some(3),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("capture_stream_latest"));
        assert!(json.contains("duration_ms"));
        assert!(json.contains("max_frames"));
        assert!(!json.contains("window_title"));
    }
}
