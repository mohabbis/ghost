//! Vision / OCR fallback when Accessibility resolution is insufficient.
//!
//! Order: AX lookup → (on failure / weak tree) ScreenCaptureKit latest frame
//! (bounded stream, still fallback) + OCR → coordinate click. Never treat
//! framework names (Electron/Flutter) as automatic vision targets — use
//! [`crate::runtime::locator::AxQuality`].

use crate::core::ocr::{self, OcrResult};
use crate::core::vision;
use crate::runtime::capture::{self, CaptureError};
use crate::runtime::locator::AxQuality;
use enigo::{Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisionFallbackError {
    NoSearchText,
    Capture(CaptureError),
    Ocr(String),
    NotFound {
        needle: String,
        recognized: Vec<String>,
    },
    Ambiguous(usize),
    Click(String),
}

impl std::fmt::Display for VisionFallbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSearchText => write!(
                f,
                "vision fallback needs target.title (or identifier) text to search"
            ),
            Self::Capture(e) => write!(f, "{e}"),
            Self::Ocr(e) => write!(f, "OCR failed: {e}"),
            Self::NotFound { needle, recognized } => write!(
                f,
                "OCR text '{needle}' not found; recognized: {recognized:?}"
            ),
            Self::Ambiguous(n) => write!(f, "ambiguous OCR text match ({n} hits)"),
            Self::Click(e) => write!(f, "vision click failed: {e}"),
        }
    }
}

/// Whether an AX failure should attempt OCR/visual fallback when search text exists.
pub fn should_attempt_vision_for_error(
    ax_error_kind: VisionAxFailure,
    has_search_text: bool,
) -> bool {
    if !has_search_text {
        return false;
    }
    matches!(
        ax_error_kind,
        VisionAxFailure::NotFound
            | VisionAxFailure::InsufficientAx
            | VisionAxFailure::Ambiguous
            | VisionAxFailure::HelperUnavailable
    )
}

/// Narrow AX failure classification so this module stays independent of [`super::semantic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisionAxFailure {
    NotFound,
    InsufficientAx,
    Ambiguous,
    HelperUnavailable,
    Other,
}

/// True when a successful AX resolve is still too weak and vision may help.
pub fn should_prefer_vision_after_resolve(quality: AxQuality, has_search_text: bool) -> bool {
    has_search_text && quality.prefer_vision_fallback() && !quality.sufficient_for_action()
}

/// Pure: pick OCR hits matching `needle`.
pub fn match_ocr_text<'a>(
    results: &'a [OcrResult],
    needle: &str,
    fuzzy: bool,
) -> Result<&'a OcrResult, VisionFallbackError> {
    let needle_trim = needle.trim();
    if needle_trim.is_empty() {
        return Err(VisionFallbackError::NoSearchText);
    }
    let needle_lower = needle_trim.to_lowercase();
    let hits: Vec<&OcrResult> = results
        .iter()
        .filter(|res| {
            let text = res.text.trim();
            if fuzzy {
                text.to_lowercase().contains(&needle_lower)
            } else {
                text == needle_trim || text.eq_ignore_ascii_case(needle_trim)
            }
        })
        .collect();
    match hits.len() {
        0 => Err(VisionFallbackError::NotFound {
            needle: needle_trim.into(),
            recognized: results.iter().map(|r| r.text.clone()).collect(),
        }),
        1 => Ok(hits[0]),
        n => Err(VisionFallbackError::Ambiguous(n)),
    }
}

/// Convert a Vision normalized box (origin bottom-left on macOS) to screen pixels.
pub fn ocr_center_screen_point(res: &OcrResult, display_w: i32, display_h: i32) -> (i32, i32) {
    let center_x = (res.x + res.w / 2.0) * display_w as f32;
    #[cfg(target_os = "macos")]
    let center_y = (1.0 - (res.y + res.h / 2.0)) * display_h as f32;
    #[cfg(not(target_os = "macos"))]
    let center_y = (res.y + res.h / 2.0) * display_h as f32;
    (center_x.round() as i32, center_y.round() as i32)
}

pub fn vision_fingerprint(res: &OcrResult, screen_x: i32, screen_y: i32) -> String {
    format!(
        "ocr|{}|{:.3},{:.3},{:.3},{:.3}|{},{}",
        res.text.trim(),
        res.x,
        res.y,
        res.w,
        res.h,
        screen_x,
        screen_y
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisionHit {
    pub text: String,
    pub screen_x: i32,
    pub screen_y: i32,
    pub fingerprint: String,
}

/// Capture (bounded stream latest → still → legacy) + OCR + unique text match.
pub fn resolve_text(
    needle: &str,
    bundle_id: Option<&str>,
    window_title: Option<&str>,
    fuzzy: bool,
) -> Result<VisionHit, VisionFallbackError> {
    let needle = needle.trim();
    if needle.is_empty() {
        return Err(VisionFallbackError::NoSearchText);
    }

    let bytes = capture::capture_latest_frame_bytes(bundle_id, window_title)
        .map_err(VisionFallbackError::Capture)?;

    let results = ocr::run_ocr(&bytes).map_err(|e| VisionFallbackError::Ocr(e.to_string()))?;
    let hit = match_ocr_text(&results, needle, fuzzy)?;
    let (display_w, display_h) = vision::display_bounds();
    let (screen_x, screen_y) = ocr_center_screen_point(hit, display_w, display_h);
    Ok(VisionHit {
        text: hit.text.clone(),
        screen_x,
        screen_y,
        fingerprint: vision_fingerprint(hit, screen_x, screen_y),
    })
}

fn click_screen_point(x: i32, y: i32) -> Result<(), VisionFallbackError> {
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| VisionFallbackError::Click(e.to_string()))?;
    enigo
        .move_mouse(x, y, Coordinate::Abs)
        .map_err(|e| VisionFallbackError::Click(e.to_string()))?;
    enigo
        .button(Button::Left, Direction::Click)
        .map_err(|e| VisionFallbackError::Click(e.to_string()))?;
    Ok(())
}

/// Resolve via OCR and click the unique text hit (coordinate fallback after AX).
pub fn focus_via_ocr(
    needle: &str,
    bundle_id: Option<&str>,
    window_title: Option<&str>,
) -> Result<VisionHit, VisionFallbackError> {
    let hit = resolve_text(needle, bundle_id, window_title, true)?;
    click_screen_point(hit.screen_x, hit.screen_y)?;
    Ok(hit)
}

/// Type into the focused field after a vision (or AX) focus.
pub fn type_text(text: &str) -> Result<(), VisionFallbackError> {
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| VisionFallbackError::Click(e.to_string()))?;
    enigo
        .text(text)
        .map_err(|e| VisionFallbackError::Click(format!("type text failed: {e}")))?;
    Ok(())
}

/// OCR-click a labeled control, then type `value` (set_value vision fallback).
pub fn set_value_via_ocr(
    needle: &str,
    value: &str,
    bundle_id: Option<&str>,
    window_title: Option<&str>,
) -> Result<VisionHit, VisionFallbackError> {
    let hit = focus_via_ocr(needle, bundle_id, window_title)?;
    std::thread::sleep(std::time::Duration::from_millis(120));
    type_text(value)?;
    Ok(hit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(text: &str, x: f32, y: f32) -> OcrResult {
        OcrResult {
            text: text.into(),
            x,
            y,
            w: 0.1,
            h: 0.05,
        }
    }

    #[test]
    fn match_ocr_requires_unique_hit() {
        let results = vec![sample("Save", 0.1, 0.2), sample("Cancel", 0.3, 0.2)];
        assert_eq!(
            match_ocr_text(&results, "Save", false).unwrap().text,
            "Save"
        );
        assert!(matches!(
            match_ocr_text(
                &[sample("Save", 0.1, 0.2), sample("Save As", 0.4, 0.2)],
                "Save",
                true
            ),
            Err(VisionFallbackError::Ambiguous(2))
        ));
    }

    #[test]
    fn should_attempt_vision_needs_title() {
        assert!(!should_attempt_vision_for_error(
            VisionAxFailure::NotFound,
            false
        ));
        assert!(should_attempt_vision_for_error(
            VisionAxFailure::NotFound,
            true
        ));
        assert!(!should_attempt_vision_for_error(
            VisionAxFailure::Other,
            true
        ));
    }

    #[test]
    fn prefer_vision_after_weak_unique_role_only() {
        let weak = AxQuality {
            score: 30,
            actionable: true,
            unique: true,
            has_identifier: false,
            has_role: true,
            has_title: false,
        };
        assert!(should_prefer_vision_after_resolve(weak, true));
        let strong = AxQuality {
            score: 100,
            actionable: true,
            unique: true,
            has_identifier: true,
            has_role: true,
            has_title: true,
        };
        assert!(!should_prefer_vision_after_resolve(strong, true));
    }

    #[test]
    fn ocr_center_is_within_display() {
        let res = sample("Hi", 0.0, 0.0);
        let (x, y) = ocr_center_screen_point(&res, 1000, 800);
        assert!((0..=1000).contains(&x));
        assert!((0..=800).contains(&y));
    }
}
