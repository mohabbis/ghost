//! Vision / OCR / template fallback when Accessibility resolution is insufficient.
//!
//! Resolution order after AX fails or scores weak:
//!
//! 1. ScreenCaptureKit latest frame + OCR
//! 2. Opt-in template match (`core/template_match`)
//! 3. Coordinate click
//!
//! Never treat framework names (Electron/Flutter) as automatic vision targets —
//! use [`crate::runtime::locator::AxQuality`]. Template fragments are opt-in on
//! [`crate::runtime::semantic::UiTarget::template_png`] only — no ambient
//! screenshot retention on this path.

use crate::core::ocr::{self, OcrResult};
use crate::core::template_match::{self, DEFAULT_MIN_SCORE};
use crate::core::vision;
use crate::runtime::capture::{self, CaptureError};
use crate::runtime::locator::AxQuality;
use enigo::{Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};
use image::GenericImageView;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisionFallbackError {
    NoSearchText,
    NoTemplate,
    Capture(CaptureError),
    Ocr(String),
    Template(String),
    NotFound {
        needle: String,
        recognized: Vec<String>,
    },
    TemplateNotFound {
        score: Option<String>,
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
            Self::NoTemplate => write!(
                f,
                "template fallback needs an opt-in target.template_png fragment"
            ),
            Self::Capture(e) => write!(f, "{e}"),
            Self::Ocr(e) => write!(f, "OCR failed: {e}"),
            Self::Template(e) => write!(f, "template match failed: {e}"),
            Self::NotFound { needle, recognized } => write!(
                f,
                "OCR text '{needle}' not found; recognized: {recognized:?}"
            ),
            Self::TemplateNotFound { score } => match score {
                Some(s) => write!(f, "template not found above threshold (best {s})"),
                None => write!(f, "template not found in capture"),
            },
            Self::Ambiguous(n) => write!(f, "ambiguous OCR text match ({n} hits)"),
            Self::Click(e) => write!(f, "vision click failed: {e}"),
        }
    }
}

/// Whether an AX failure should attempt OCR/template fallback.
pub fn should_attempt_vision_for_error(
    ax_error_kind: VisionAxFailure,
    has_search_text: bool,
    has_template: bool,
) -> bool {
    if !has_search_text && !has_template {
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

/// True when a successful AX resolve is still too weak and vision/template may help.
pub fn should_prefer_vision_after_resolve(
    quality: AxQuality,
    has_search_text: bool,
    has_template: bool,
) -> bool {
    (has_search_text || has_template)
        && quality.prefer_vision_fallback()
        && !quality.sufficient_for_action()
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

pub fn template_fingerprint(score: f32, screen_x: i32, screen_y: i32) -> String {
    format!("template|{score:.3}|{screen_x},{screen_y}")
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

/// Locate an opt-in template PNG in a **full-display** capture, then return
/// the click center in screen pixels.
///
/// Full-display capture is intentional so match coordinates map to absolute
/// screen points for [`click_screen_point`]. Window-scoped crops would mis-click.
///
/// Not business-effect proof (ADR-0007) — pixel similarity only.
pub fn resolve_via_template(template_png: &[u8]) -> Result<VisionHit, VisionFallbackError> {
    if template_png.is_empty() {
        return Err(VisionFallbackError::NoTemplate);
    }
    let template = image::load_from_memory(template_png)
        .map_err(|e| VisionFallbackError::Template(e.to_string()))?;

    // Full display: template coords must be screen-absolute for enigo clicks.
    let bytes =
        capture::capture_latest_frame_bytes(None, None).map_err(VisionFallbackError::Capture)?;
    let haystack = image::load_from_memory(&bytes)
        .map_err(|e| VisionFallbackError::Template(e.to_string()))?;

    let m = template_match::find_template(&haystack, &template)
        .ok_or(VisionFallbackError::TemplateNotFound { score: None })?;
    if m.score < DEFAULT_MIN_SCORE {
        return Err(VisionFallbackError::TemplateNotFound {
            score: Some(format!("{:.3}", m.score)),
        });
    }
    let (tw, th) = template.dimensions();
    let screen_x = m.x as i32 + (tw as i32) / 2;
    let screen_y = m.y as i32 + (th as i32) / 2;
    Ok(VisionHit {
        text: format!("template:{:.2}", m.score),
        screen_x,
        screen_y,
        fingerprint: template_fingerprint(m.score, screen_x, screen_y),
    })
}

/// Pure helper for tests: match template bytes inside already-captured PNG bytes.
pub fn match_template_in_png(
    haystack_png: &[u8],
    template_png: &[u8],
) -> Result<VisionHit, VisionFallbackError> {
    if template_png.is_empty() {
        return Err(VisionFallbackError::NoTemplate);
    }
    let template = image::load_from_memory(template_png)
        .map_err(|e| VisionFallbackError::Template(e.to_string()))?;
    let haystack = image::load_from_memory(haystack_png)
        .map_err(|e| VisionFallbackError::Template(e.to_string()))?;
    let m = template_match::find_template(&haystack, &template)
        .ok_or(VisionFallbackError::TemplateNotFound { score: None })?;
    if m.score < DEFAULT_MIN_SCORE {
        return Err(VisionFallbackError::TemplateNotFound {
            score: Some(format!("{:.3}", m.score)),
        });
    }
    let (tw, th) = template.dimensions();
    let screen_x = m.x as i32 + (tw as i32) / 2;
    let screen_y = m.y as i32 + (th as i32) / 2;
    Ok(VisionHit {
        text: format!("template:{:.2}", m.score),
        screen_x,
        screen_y,
        fingerprint: template_fingerprint(m.score, screen_x, screen_y),
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

/// Resolve via opt-in template match and click the match center.
pub fn focus_via_template(template_png: &[u8]) -> Result<VisionHit, VisionFallbackError> {
    let hit = resolve_via_template(template_png)?;
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

/// Template-click a control, then type `value` (after OCR failed or no title).
pub fn set_value_via_template(
    template_png: &[u8],
    value: &str,
) -> Result<VisionHit, VisionFallbackError> {
    let hit = focus_via_template(template_png)?;
    std::thread::sleep(std::time::Duration::from_millis(120));
    type_text(value)?;
    Ok(hit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgb, RgbImage};

    fn sample(text: &str, x: f32, y: f32) -> OcrResult {
        OcrResult {
            text: text.into(),
            x,
            y,
            w: 0.1,
            h: 0.05,
        }
    }

    /// Block-sized checkerboard so a 4× downsample (see `template_match`) keeps contrast.
    fn distinct_template(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, y| {
            if ((x / 4) + (y / 4)) % 2 == 0 {
                Rgb([240, 20, 20])
            } else {
                Rgb([10, 10, 250])
            }
        })
    }

    fn encode_png(img: &RgbImage) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img.clone())
            .write_to(&mut cursor, image::ImageFormat::Png)
            .expect("png encode");
        cursor.into_inner()
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
    fn should_attempt_vision_needs_title_or_template() {
        assert!(!should_attempt_vision_for_error(
            VisionAxFailure::NotFound,
            false,
            false
        ));
        assert!(should_attempt_vision_for_error(
            VisionAxFailure::NotFound,
            true,
            false
        ));
        assert!(should_attempt_vision_for_error(
            VisionAxFailure::NotFound,
            false,
            true
        ));
        assert!(!should_attempt_vision_for_error(
            VisionAxFailure::Other,
            true,
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
        assert!(should_prefer_vision_after_resolve(weak, true, false));
        assert!(should_prefer_vision_after_resolve(weak, false, true));
        assert!(!should_prefer_vision_after_resolve(weak, false, false));
        let strong = AxQuality {
            score: 100,
            actionable: true,
            unique: true,
            has_identifier: true,
            has_role: true,
            has_title: true,
        };
        assert!(!should_prefer_vision_after_resolve(strong, true, true));
    }

    #[test]
    fn ocr_center_is_within_display() {
        let res = sample("Hi", 0.0, 0.0);
        let (x, y) = ocr_center_screen_point(&res, 1000, 800);
        assert!((0..=1000).contains(&x));
        assert!((0..=800).contains(&y));
    }

    #[test]
    fn match_template_in_png_finds_embedded_control() {
        // Mirror core/template_match: noisy canvas + block template at DOWNSAMPLE multiple.
        let mut canvas = RgbImage::from_fn(200, 150, |x, y| {
            Rgb([
                (30 + (x % 50)) as u8,
                (40 + (y % 40)) as u8,
                (50 + ((x + y) % 60)) as u8,
            ])
        });
        let template = distinct_template(24, 24);
        for y in 0..template.height() {
            for x in 0..template.width() {
                canvas.put_pixel(80 + x, 48 + y, *template.get_pixel(x, y));
            }
        }
        let haystack = encode_png(&canvas);
        let needle = encode_png(&template);
        let hit = match_template_in_png(&haystack, &needle).expect("template hit");
        // Center of 24x24 at (80,48) ≈ (92, 60); downsample may shift a few px.
        assert!((hit.screen_x - 92).abs() <= 12, "x={}", hit.screen_x);
        assert!((hit.screen_y - 60).abs() <= 12, "y={}", hit.screen_y);
        assert!(hit.fingerprint.starts_with("template|"));
    }

    #[test]
    fn empty_template_is_rejected() {
        assert!(matches!(
            match_template_in_png(&[1, 2, 3], &[]),
            Err(VisionFallbackError::NoTemplate)
        ));
    }
}
