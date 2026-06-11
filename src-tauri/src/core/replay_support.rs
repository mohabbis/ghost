//! Shared replay plumbing used by both platform replay engines:
//! pause/cancel-aware control flow, timestamp-based pacing, and element
//! re-resolution (self-healing) helpers.
//!
//! Everything here is platform-agnostic and unit-tested; the platform
//! modules supply only the raw "what element is at (x, y)" lookup.

use crate::core::events::ElementInfo;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// How often replay re-checks the stop/pause flags while sleeping or paused.
const POLL_MS: u64 = 25;

/// Pacing gaps derived from recorded timestamps are capped so a workflow
/// recorded across a coffee break doesn't make replay hang for minutes.
pub const MAX_PACING_GAP_MS: u64 = 10_000;

/// Block while replay is paused. Returns `false` if replay was cancelled
/// (stop flag set) either before or during the pause, `true` to proceed.
pub fn check_continue(stop: &AtomicBool, paused: &AtomicBool) -> bool {
    loop {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        if !paused.load(Ordering::Relaxed) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(POLL_MS));
    }
}

/// Sleep for `ms`, waking early on cancel and not counting down while paused.
/// Returns `false` if replay was cancelled during the sleep.
pub fn interruptible_sleep(ms: u64, stop: &AtomicBool, paused: &AtomicBool) -> bool {
    let mut remaining = ms;
    while remaining > 0 {
        if !check_continue(stop, paused) {
            return false;
        }
        let slice = remaining.min(POLL_MS);
        std::thread::sleep(Duration::from_millis(slice));
        remaining -= slice;
    }
    check_continue(stop, paused)
}

/// Gap to sleep before an event so replay mirrors the recorded rhythm.
/// Returns 0 when either timestamp is missing (pre-pacing recordings) or
/// out of order; clamps long idle periods to `MAX_PACING_GAP_MS`.
pub fn pacing_gap_ms(prev_ts: Option<u64>, current_ts: Option<u64>) -> u64 {
    match (prev_ts, current_ts) {
        (Some(prev), Some(cur)) if cur > prev => (cur - prev).min(MAX_PACING_GAP_MS),
        _ => 0,
    }
}

/// Does the live element `found` match the recorded `target` descriptor?
/// Prefers the stable automation identifier when both sides have one.
pub fn descriptor_matches(target: &ElementInfo, found: &ElementInfo) -> bool {
    if let (Some(t_id), Some(f_id)) = (&target.identifier, &found.identifier) {
        if !t_id.is_empty() {
            return t_id == f_id;
        }
    }
    if target.role.is_empty() || !target.role.eq_ignore_ascii_case(&found.role) {
        return false;
    }
    if !target.name.is_empty() {
        return target.name.eq_ignore_ascii_case(&found.name);
    }
    if target.app.is_empty() || target.app == "Unknown" {
        true
    } else {
        target.app.eq_ignore_ascii_case(&found.app)
    }
}

/// Outward spiral used when re-resolving a moved element: four rings of
/// eight directions around the recorded point.
pub const SEARCH_RADII: [i32; 4] = [30, 70, 140, 260];
pub const SEARCH_DIRS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Re-resolve where to click for a recorded element using a platform lookup
/// closure. Returns `None` when no matching element is found anywhere near
/// the recorded point (callers decide whether to fall back or retry).
pub fn try_resolve_click_point<F>(
    target: &ElementInfo,
    rx: i32,
    ry: i32,
    lookup: F,
) -> Option<(i32, i32)>
where
    F: Fn(i32, i32) -> Option<ElementInfo>,
{
    if let Some(found) = lookup(rx, ry) {
        if descriptor_matches(target, &found) {
            return Some((rx, ry));
        }
    }

    for r in SEARCH_RADII {
        for (dx, dy) in SEARCH_DIRS {
            let (px, py) = (rx + dx * r, ry + dy * r);
            if px < 0 || py < 0 {
                continue;
            }
            if let Some(found) = lookup(px, py) {
                if descriptor_matches(target, &found) {
                    return Some((px, py));
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn info(role: &str, name: &str, app: &str) -> ElementInfo {
        ElementInfo {
            role: role.into(),
            name: name.into(),
            app: app.into(),
            fallback_coords: Some((0, 0)),
            ..Default::default()
        }
    }

    // ── descriptor_matches ────────────────────────────────────────────────

    #[test]
    fn matches_same_role_and_name_case_insensitively() {
        let target = info("AXButton", "Save", "Notes");
        let found = info("axbutton", "save", "Notes");
        assert!(descriptor_matches(&target, &found));
    }

    #[test]
    fn rejects_different_name() {
        let target = info("AXButton", "Save", "Notes");
        let found = info("AXButton", "Cancel", "Notes");
        assert!(!descriptor_matches(&target, &found));
    }

    #[test]
    fn rejects_different_role() {
        let target = info("AXButton", "Save", "Notes");
        let found = info("AXTextField", "Save", "Notes");
        assert!(!descriptor_matches(&target, &found));
    }

    #[test]
    fn nameless_target_falls_back_to_role_plus_app() {
        let target = info("AXButton", "", "Notes");
        assert!(descriptor_matches(
            &target,
            &info("AXButton", "whatever", "Notes")
        ));
        assert!(!descriptor_matches(
            &target,
            &info("AXButton", "whatever", "Safari")
        ));
    }

    #[test]
    fn nameless_target_unknown_app_matches_on_role_only() {
        let target = info("AXButton", "", "Unknown");
        assert!(descriptor_matches(
            &target,
            &info("AXButton", "anything", "AnyApp")
        ));
    }

    #[test]
    fn empty_target_role_never_matches() {
        let target = info("", "Save", "Notes");
        assert!(!descriptor_matches(&target, &info("", "Save", "Notes")));
    }

    #[test]
    fn stable_identifier_wins_over_name() {
        let mut target = info("AXButton", "Save", "Notes");
        target.identifier = Some("save-btn".into());
        let mut found = info("AXButton", "Save (2 left)", "Notes");
        found.identifier = Some("save-btn".into());
        // Name changed but identifier is stable → still a match.
        assert!(descriptor_matches(&target, &found));

        found.identifier = Some("other-btn".into());
        assert!(!descriptor_matches(&target, &found));
    }

    // ── pacing ────────────────────────────────────────────────────────────

    #[test]
    fn pacing_handles_missing_and_unordered_timestamps() {
        assert_eq!(pacing_gap_ms(None, Some(100)), 0);
        assert_eq!(pacing_gap_ms(Some(100), None), 0);
        assert_eq!(pacing_gap_ms(Some(200), Some(100)), 0); // out of order
        assert_eq!(pacing_gap_ms(Some(100), Some(350)), 250);
    }

    #[test]
    fn pacing_clamps_long_idle_gaps() {
        assert_eq!(
            pacing_gap_ms(Some(0), Some(120_000)),
            super::MAX_PACING_GAP_MS
        );
    }

    // ── control flow ──────────────────────────────────────────────────────

    #[test]
    fn sleep_aborts_when_cancelled() {
        let stop = Arc::new(AtomicBool::new(false));
        let paused = AtomicBool::new(false);

        let stop2 = stop.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            stop2.store(true, Ordering::Relaxed);
        });

        let started = std::time::Instant::now();
        let completed = interruptible_sleep(5_000, &stop, &paused);
        assert!(!completed, "sleep should report cancellation");
        assert!(
            started.elapsed() < Duration::from_millis(2_000),
            "cancel must interrupt the sleep promptly"
        );
    }

    #[test]
    fn pause_blocks_until_resumed() {
        let stop = AtomicBool::new(false);
        let paused = Arc::new(AtomicBool::new(true));

        let paused2 = paused.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            paused2.store(false, Ordering::Relaxed);
        });

        let started = std::time::Instant::now();
        assert!(check_continue(&stop, &paused));
        assert!(
            started.elapsed() >= Duration::from_millis(50),
            "check_continue must actually block while paused"
        );
    }

    // ── self-heal resolution ──────────────────────────────────────────────

    #[test]
    fn resolves_to_recorded_point_when_element_unmoved() {
        let target = info("AXButton", "Save", "Notes");
        let at_point = target.clone();
        let resolved = try_resolve_click_point(&target, 10, 10, |x, y| {
            (x == 10 && y == 10).then(|| at_point.clone())
        });
        assert_eq!(resolved, Some((10, 10)));
    }

    #[test]
    fn finds_moved_element_nearby() {
        let target = info("AXButton", "Save", "Notes");
        let moved = target.clone();
        // Element now lives 70px to the right of where it was recorded.
        let resolved = try_resolve_click_point(&target, 100, 100, |x, y| {
            (x == 170 && y == 100).then(|| moved.clone())
        });
        assert_eq!(resolved, Some((170, 100)));
    }

    #[test]
    fn returns_none_when_element_gone() {
        let target = info("AXButton", "Save", "Notes");
        let resolved = try_resolve_click_point(&target, 100, 100, |_, _| None);
        assert_eq!(resolved, None);
    }
}
