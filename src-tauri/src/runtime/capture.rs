//! ScreenCaptureKit still-frame and bounded stream capture via GhostAXHelper.
//!
//! Capture is a separate permission surface from Accessibility. When Screen
//! Recording is denied, AX-only automation must keep working.
//!
//! Still-frame (`capture_still`) remains the simple default. Bounded stream
//! (`capture_stream_latest`) is an opt-in reliability upgrade that samples a
//! short SCStream and returns the latest complete frame — never ambient
//! observation.
//!
//! Optional `budget_ms` on helper requests lets ActionStep timeouts interrupt
//! mid-op cooperatively inside GhostAXHelper (see `helper_budget`).

use crate::runtime::helper_budget::{self, HelperBudget};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use std::io::{BufRead, BufReader, Write};
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};

/// Which request-scoped capture API produced a frame (never ambient).
///
/// Recorded on OCR/template step evidence — never stores screenshot bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePath {
    /// ScreenCaptureKit single still frame (`capture_still`).
    Still,
    /// Bounded SCStream sample → latest complete frame (`capture_stream_latest`).
    Stream,
    /// Legacy `screencapture` / platform path when the SCK helper is unavailable.
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    PermissionDenied(String),
    HelperUnavailable(String),
    /// Cooperative mid-op budget exhausted inside the helper (or before spawn).
    TimedOut(String),
    Failed(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermissionDenied(d) => write!(f, "screen recording denied: {d}"),
            Self::HelperUnavailable(d) => write!(f, "capture helper unavailable: {d}"),
            Self::TimedOut(d) => write!(f, "capture timed out: {d}"),
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

    /// Shorten duration to a helper budget (budget can only shrink; hard caps stay).
    pub fn clamp_to_budget(self, budget: HelperBudget) -> Result<Self, CaptureError> {
        let mut opts = self.clamped();
        let duration = budget.clamp_duration_ms(opts.duration_ms);
        if duration < Self::MIN_DURATION_MS {
            return Err(CaptureError::TimedOut(
                helper_budget::HELPER_BUDGET_EXCEEDED.into(),
            ));
        }
        opts.duration_ms = duration;
        Ok(opts)
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
    /// Wall-clock ms from helper request start; cooperative mid-op deadline.
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_ms: Option<u32>,
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

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn map_helper_response(resp: CaptureResponse) -> Result<CaptureResponse, CaptureError> {
    if !resp.ok && helper_budget::is_helper_budget_timeout(&resp.detail) {
        return Err(CaptureError::TimedOut(resp.detail));
    }
    if !resp.ok
        && (resp.detail.contains("screen recording denied")
            || resp.detail.contains("ScreenCaptureKit")
                && resp.detail.to_lowercase().contains("denied"))
    {
        return Err(CaptureError::PermissionDenied(resp.detail));
    }
    Ok(resp)
}

fn ensure_budget(budget: Option<HelperBudget>) -> Result<(), CaptureError> {
    if let Some(b) = budget
        && b.is_exhausted()
    {
        return Err(CaptureError::TimedOut(
            helper_budget::HELPER_BUDGET_EXCEEDED.into(),
        ));
    }
    Ok(())
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
        map_helper_response(resp)
    }
}

fn frame_from_response(
    resp: CaptureResponse,
    dest_png: &Path,
    op: &str,
) -> Result<StillFrame, CaptureError> {
    if !resp.ok {
        if helper_budget::is_helper_budget_timeout(&resp.detail) {
            return Err(CaptureError::TimedOut(resp.detail));
        }
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
        budget_ms: None,
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
    capture_still_with_budget(dest_png, bundle_id, window_title, None)
}

/// Like [`capture_still`], with an optional cooperative mid-op budget.
pub fn capture_still_with_budget(
    dest_png: &Path,
    bundle_id: Option<&str>,
    window_title: Option<&str>,
    budget: Option<HelperBudget>,
) -> Result<StillFrame, CaptureError> {
    ensure_budget(budget)?;
    let resp = call_helper(&CaptureRequest {
        op: "capture_still".into(),
        path: Some(dest_png.to_string_lossy().into_owned()),
        bundle_id: bundle_id.map(str::to_string),
        window_title: window_title.map(str::to_string),
        duration_ms: None,
        max_frames: None,
        budget_ms: budget.map(|b| b.budget_ms),
    })?;
    frame_from_response(resp, dest_png, "capture_still")
}

/// Bounded ScreenCaptureKit stream sample → latest complete frame.
///
/// Opt-in reliability path for OCR: short-lived, hard-capped, request-scoped.
/// Prefer [`capture_still`] when a single snapshot is enough.
///
/// Hard caps remain ≤2s / 8 frames; a helper budget can only shorten `duration_ms`.
pub fn capture_stream_latest(
    dest_png: &Path,
    bundle_id: Option<&str>,
    window_title: Option<&str>,
    opts: StreamCaptureOpts,
) -> Result<StillFrame, CaptureError> {
    capture_stream_latest_with_budget(dest_png, bundle_id, window_title, opts, None)
}

/// Like [`capture_stream_latest`], clamping stream duration to `budget` when set.
pub fn capture_stream_latest_with_budget(
    dest_png: &Path,
    bundle_id: Option<&str>,
    window_title: Option<&str>,
    opts: StreamCaptureOpts,
    budget: Option<HelperBudget>,
) -> Result<StillFrame, CaptureError> {
    ensure_budget(budget)?;
    let opts = match budget {
        Some(b) => opts.clamp_to_budget(b)?,
        None => opts.clamped(),
    };
    let resp = call_helper(&CaptureRequest {
        op: "capture_stream_latest".into(),
        path: Some(dest_png.to_string_lossy().into_owned()),
        bundle_id: bundle_id.map(str::to_string),
        window_title: window_title.map(str::to_string),
        duration_ms: Some(opts.duration_ms),
        max_frames: Some(opts.max_frames),
        budget_ms: budget.map(|b| b.budget_ms),
    })?;
    frame_from_response(resp, dest_png, "capture_stream_latest")
}

/// Prefer ScreenCaptureKit helper; fall back to legacy `screencapture` / platform path.
///
/// Returns which path produced the bytes (`still` or `legacy`). Never retains
/// the frame beyond this call site's use.
pub fn capture_still_bytes(
    bundle_id: Option<&str>,
    window_title: Option<&str>,
) -> Result<(Vec<u8>, CapturePath), CaptureError> {
    capture_still_bytes_with_budget(bundle_id, window_title, None)
}

/// Like [`capture_still_bytes`] with a cooperative mid-op budget.
pub fn capture_still_bytes_with_budget(
    bundle_id: Option<&str>,
    window_title: Option<&str>,
    budget: Option<HelperBudget>,
) -> Result<(Vec<u8>, CapturePath), CaptureError> {
    let dest = std::env::temp_dir().join(format!("ghost_sck_{}.png", uuid::Uuid::new_v4()));
    match capture_still_with_budget(&dest, bundle_id, window_title, budget) {
        Ok(frame) => {
            let bytes =
                std::fs::read(&frame.path).map_err(|e| CaptureError::Failed(e.to_string()))?;
            let _ = std::fs::remove_file(&frame.path);
            Ok((bytes, CapturePath::Still))
        }
        Err(CaptureError::TimedOut(d)) => Err(CaptureError::TimedOut(d)),
        Err(CaptureError::HelperUnavailable(_)) | Err(CaptureError::PermissionDenied(_)) => {
            crate::core::vision::capture_screenshot()
                .map(|bytes| (bytes, CapturePath::Legacy))
                .map_err(|e| CaptureError::Failed(e.to_string()))
        }
        Err(e) => {
            // Helper present but capture failed — try legacy path once.
            match crate::core::vision::capture_screenshot() {
                Ok(bytes) => Ok((bytes, CapturePath::Legacy)),
                Err(_) => Err(e),
            }
        }
    }
}

/// Latest frame for OCR fallback: bounded stream first, then still, then legacy.
///
/// Stream is the reliability upgrade; still-frame remains the fallback when the
/// stream op is unavailable or fails. Never starts ambient capture.
///
/// The [`CapturePath`] reports which tier succeeded (`stream` / `still` /
/// `legacy`) so receipts can record it without storing screenshot bytes.
pub fn capture_latest_frame_bytes(
    bundle_id: Option<&str>,
    window_title: Option<&str>,
) -> Result<(Vec<u8>, CapturePath), CaptureError> {
    capture_latest_frame_bytes_with_opts(bundle_id, window_title, StreamCaptureOpts::default())
}

/// Same as [`capture_latest_frame_bytes`] with explicit stream bounds.
pub fn capture_latest_frame_bytes_with_opts(
    bundle_id: Option<&str>,
    window_title: Option<&str>,
    opts: StreamCaptureOpts,
) -> Result<(Vec<u8>, CapturePath), CaptureError> {
    capture_latest_frame_bytes_with_budget(bundle_id, window_title, opts, None)
}

/// Same as [`capture_latest_frame_bytes_with_opts`] with a cooperative mid-op budget.
pub fn capture_latest_frame_bytes_with_budget(
    bundle_id: Option<&str>,
    window_title: Option<&str>,
    opts: StreamCaptureOpts,
    budget: Option<HelperBudget>,
) -> Result<(Vec<u8>, CapturePath), CaptureError> {
    let dest = std::env::temp_dir().join(format!("ghost_sck_stream_{}.png", uuid::Uuid::new_v4()));
    match capture_stream_latest_with_budget(&dest, bundle_id, window_title, opts, budget) {
        Ok(frame) => {
            let bytes =
                std::fs::read(&frame.path).map_err(|e| CaptureError::Failed(e.to_string()))?;
            let _ = std::fs::remove_file(&frame.path);
            Ok((bytes, CapturePath::Stream))
        }
        Err(CaptureError::TimedOut(d)) => Err(CaptureError::TimedOut(d)),
        Err(CaptureError::HelperUnavailable(_)) | Err(CaptureError::PermissionDenied(_)) => {
            // No stream path — degrade to still / legacy without inventing capture.
            capture_still_bytes_with_budget(bundle_id, window_title, budget)
        }
        Err(_) => {
            // Stream helper present but sample failed — still-frame once, then legacy.
            capture_still_bytes_with_budget(bundle_id, window_title, budget)
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
            budget_ms: Some(1_200),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("capture_stream_latest"));
        assert!(json.contains("duration_ms"));
        assert!(json.contains("max_frames"));
        assert!(json.contains("budget_ms"));
        assert!(!json.contains("window_title"));
    }

    #[test]
    fn stream_budget_shortens_but_respects_hard_caps() {
        let opts = StreamCaptureOpts {
            duration_ms: 50_000,
            max_frames: 99,
        };
        let clamped = opts.clamp_to_budget(HelperBudget::new(300)).unwrap();
        assert_eq!(clamped.duration_ms, 300);
        assert_eq!(clamped.max_frames, StreamCaptureOpts::MAX_FRAMES);
        assert!(clamped.duration_ms <= StreamCaptureOpts::MAX_DURATION_MS);
    }

    #[test]
    fn exhausted_budget_fails_before_helper() {
        let dest = std::env::temp_dir().join("ghost_budget_exhausted.png");
        let err =
            capture_still_with_budget(&dest, None, None, Some(HelperBudget::new(0))).unwrap_err();
        assert!(matches!(err, CaptureError::TimedOut(_)));
    }

    #[test]
    fn stream_budget_below_min_fails_closed() {
        let dest = std::env::temp_dir().join("ghost_budget_tiny.png");
        let err = capture_stream_latest_with_budget(
            &dest,
            None,
            None,
            StreamCaptureOpts::default(),
            Some(HelperBudget::new(10)),
        )
        .unwrap_err();
        assert!(matches!(err, CaptureError::TimedOut(_)));
    }

    #[test]
    fn map_helper_timeout_detail() {
        let resp = CaptureResponse {
            ok: false,
            detail: helper_budget::HELPER_BUDGET_EXCEEDED.into(),
            path: None,
            width: None,
            height: None,
            frames: None,
        };
        let err = map_helper_response(resp).unwrap_err();
        assert!(matches!(err, CaptureError::TimedOut(_)));
    }
    #[test]
    fn capture_path_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&CapturePath::Still).unwrap(),
            "\"still\""
        );
        assert_eq!(
            serde_json::to_string(&CapturePath::Stream).unwrap(),
            "\"stream\""
        );
        assert_eq!(
            serde_json::to_string(&CapturePath::Legacy).unwrap(),
            "\"legacy\""
        );
    }
}
