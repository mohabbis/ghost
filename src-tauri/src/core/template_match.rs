//! Lightweight, dependency-free template matching: locate a small reference
//! image (a crop taken at record time) inside a larger screenshot.
//!
//! This is the "beyond hardcoded pixel coordinates, even if the window
//! resizes" fallback for [`crate::core::replay_support`]'s resolution
//! chain — for elements a semantic (accessibility-tree) lookup can't find,
//! but whose *pixels* haven't changed.
//!
//! Deliberately pure Rust over the `image` crate already in this crate's
//! dependency tree, rather than an OpenCV binding (`opencv-rust`): OpenCV
//! requires a prebuilt system library (via pkg-config on Linux, vcpkg on
//! Windows, brew on macOS) that isn't installed anywhere in this repo's 3-OS
//! CI matrix (`rust.yml`), and adding it would make the crate unbuildable
//! for any contributor without OpenCV already on their machine. Normalized
//! cross-correlation over grayscale, downsampled images is a small,
//! self-contained implementation of the same idea, with no new dependency
//! and no system-library requirement at all.

use image::{DynamicImage, GenericImageView};

/// Shrink factor applied to both images before matching. Matching cost scales
/// with (search area) × (template area); a factor-4 downsample cuts that by
/// 16x, keeping this fast enough to run inline as a replay fallback.
const DOWNSAMPLE: u32 = 4;

/// Minimum normalized cross-correlation score (of 1.0 = perfect) to accept a
/// match. Conservative on purpose: falling through to coordinate fallback is
/// safer than clicking the wrong control.
pub const DEFAULT_MIN_SCORE: f32 = 0.80;

/// A located match, in the original (pre-downsample) pixel coordinates of
/// the `haystack` image passed to [`find_template`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemplateMatch {
    /// Top-left corner of the best-matching region.
    pub x: u32,
    pub y: u32,
    /// Normalized cross-correlation score in `[-1.0, 1.0]`.
    pub score: f32,
}

/// Find the best-matching location of `template` inside `haystack`.
///
/// Returns `None` for a degenerate template (zero-sized, or larger than the
/// haystack after downsampling) — never panics on mismatched or tiny inputs.
pub fn find_template(haystack: &DynamicImage, template: &DynamicImage) -> Option<TemplateMatch> {
    // Checked pre-downsample: `downsample`'s `.max(1)` floor would otherwise
    // turn a genuinely zero-sized template into a spurious 1x1 one.
    let (orig_tw, orig_th) = template.dimensions();
    let (orig_hw, orig_hh) = haystack.dimensions();
    if orig_tw == 0 || orig_th == 0 || orig_tw > orig_hw || orig_th > orig_hh {
        return None;
    }

    let hay_small = downsample(haystack);
    let tpl_small = downsample(template);

    let (hw, hh) = hay_small.dimensions();
    let (tw, th) = tpl_small.dimensions();
    if tw == 0 || th == 0 || tw > hw || th > hh {
        return None;
    }

    let hay_gray = hay_small.to_luma8();
    let tpl_gray = tpl_small.to_luma8();

    let mut best: Option<(u32, u32, f32)> = None;
    for y in 0..=(hh - th) {
        for x in 0..=(hw - tw) {
            let score = normalized_cross_correlation(&hay_gray, &tpl_gray, x, y);
            let is_better = match best {
                Some((_, _, best_score)) => score > best_score,
                None => true,
            };
            if is_better {
                best = Some((x, y, score));
            }
        }
    }

    best.map(|(x, y, score)| TemplateMatch {
        x: x * DOWNSAMPLE,
        y: y * DOWNSAMPLE,
        score,
    })
}

fn downsample(img: &DynamicImage) -> DynamicImage {
    let (w, h) = img.dimensions();
    let (nw, nh) = ((w / DOWNSAMPLE).max(1), (h / DOWNSAMPLE).max(1));
    // Nearest, not a blending filter (Triangle/Lanczos): a blending filter's
    // support window crosses the template's edges when it's downsampled as
    // part of a larger embedding haystack (mixing in neighboring pixels) but
    // not when the same template is downsampled standalone, diluting the
    // correlation between otherwise-identical content. Nearest maps each
    // output pixel to exactly one source pixel, so there's no cross-boundary
    // blending to cause that mismatch.
    img.resize_exact(nw, nh, image::imageops::FilterType::Nearest)
}

/// Normalized cross-correlation of `template` against the `template`-sized
/// window of `haystack` whose top-left corner is `(ox, oy)`. `1.0` is a
/// perfect match; `0.0` when either window is uniform (zero variance) and
/// therefore uncorrelatable.
fn normalized_cross_correlation(
    haystack: &image::GrayImage,
    template: &image::GrayImage,
    ox: u32,
    oy: u32,
) -> f32 {
    let (tw, th) = template.dimensions();
    let n = (tw * th) as f32;

    let mut sum_h = 0f32;
    let mut sum_t = 0f32;
    for y in 0..th {
        for x in 0..tw {
            sum_h += haystack.get_pixel(ox + x, oy + y)[0] as f32;
            sum_t += template.get_pixel(x, y)[0] as f32;
        }
    }
    let mean_h = sum_h / n;
    let mean_t = sum_t / n;

    let mut cov = 0f32;
    let mut var_h = 0f32;
    let mut var_t = 0f32;
    for y in 0..th {
        for x in 0..tw {
            let dh = haystack.get_pixel(ox + x, oy + y)[0] as f32 - mean_h;
            let dt = template.get_pixel(x, y)[0] as f32 - mean_t;
            cov += dh * dt;
            var_h += dh * dh;
            var_t += dt * dt;
        }
    }

    let denom = (var_h * var_t).sqrt();
    if denom == 0.0 {
        0.0
    } else {
        cov / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    /// A varied canvas so windows away from the embedded template don't
    /// accidentally correlate (a uniform background would make every
    /// position equally "perfect", masking real bugs). Varies in blocks of
    /// `DOWNSAMPLE` pixels rather than per-pixel, so the pattern — and thus
    /// the match location — survives the 4x downsample instead of being
    /// blurred away by it.
    fn noisy_canvas(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, y| {
            let (bx, by) = (x / DOWNSAMPLE, y / DOWNSAMPLE);
            let v = ((bx * 37 + by * 91) % 256) as u8;
            Rgb([v, v, v])
        })
    }

    fn distinct_template(w: u32, h: u32) -> RgbImage {
        // A block-patterned image that doesn't resemble `noisy_canvas`'s
        // formula anywhere, so it only matches where it's deliberately
        // embedded. Same block-size reasoning as `noisy_canvas`.
        RgbImage::from_fn(w, h, |x, y| {
            let (bx, by) = (x / DOWNSAMPLE, y / DOWNSAMPLE);
            if (bx + by) % 2 == 0 {
                Rgb([250, 10, 10])
            } else {
                Rgb([10, 10, 250])
            }
        })
    }

    fn embed(canvas: &mut RgbImage, template: &RgbImage, at_x: u32, at_y: u32) {
        for y in 0..template.height() {
            for x in 0..template.width() {
                canvas.put_pixel(at_x + x, at_y + y, *template.get_pixel(x, y));
            }
        }
    }

    #[test]
    fn finds_embedded_template_at_the_right_location() {
        let mut canvas = noisy_canvas(200, 150);
        let template = distinct_template(24, 24);
        // Embedded at a multiple of DOWNSAMPLE so it aligns with the
        // downsampled grid exactly (a click target's real position won't
        // generally do this, but the point of this test is "does the search
        // find the right neighborhood", not sub-pixel precision — a few
        // pixels of slop when clicking a real, tens-of-pixels-wide UI
        // element is harmless).
        embed(&mut canvas, &template, 80, 48);

        let haystack = DynamicImage::ImageRgb8(canvas);
        let needle = DynamicImage::ImageRgb8(template);

        let m = find_template(&haystack, &needle).expect("template must be found");
        let tolerance = 2 * DOWNSAMPLE as i32;
        assert!(
            (m.x as i32 - 80).abs() <= tolerance,
            "x={} expected near 80",
            m.x
        );
        assert!(
            (m.y as i32 - 48).abs() <= tolerance,
            "y={} expected near 48",
            m.y
        );
        assert!(m.score > DEFAULT_MIN_SCORE, "score={} too low", m.score);
    }

    #[test]
    fn template_larger_than_haystack_returns_none() {
        let haystack = DynamicImage::ImageRgb8(noisy_canvas(20, 20));
        let template = DynamicImage::ImageRgb8(distinct_template(200, 200));
        assert!(find_template(&haystack, &template).is_none());
    }

    #[test]
    fn zero_sized_template_returns_none() {
        let haystack = DynamicImage::ImageRgb8(noisy_canvas(20, 20));
        let template = DynamicImage::ImageRgb8(RgbImage::new(0, 0));
        assert!(find_template(&haystack, &template).is_none());
    }

    #[test]
    fn uniform_regions_score_zero_not_nan() {
        // Solid color everywhere: variance is zero, so NCC is defined as 0.0
        // rather than dividing by zero.
        let haystack = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 40, Rgb([100, 100, 100])));
        let template = DynamicImage::ImageRgb8(RgbImage::from_pixel(8, 8, Rgb([100, 100, 100])));
        let m = find_template(&haystack, &template).expect("degenerate but not None");
        assert_eq!(m.score, 0.0);
    }
}
