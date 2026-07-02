//! Shared replay plumbing used by both platform replay engines:
//! pause/cancel-aware control flow, timestamp-based pacing, and element
//! re-resolution (self-healing) helpers.
//!
//! Everything here is platform-agnostic and unit-tested; the platform
//! modules supply only the raw "what element is at (x, y)" lookup.

use crate::core::events::ElementInfo;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// How often replay re-checks the stop/pause flags while sleeping or paused.
const POLL_MS: u64 = 25;

/// Pacing gaps derived from recorded timestamps are capped so a workflow
/// recorded across a coffee break doesn't make replay hang for minutes.
pub const MAX_PACING_GAP_MS: u64 = 10_000;

/// How a click's target was resolved at replay time. Recorded per click so
/// every run produces an explainable trace of which locator strategy won.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ResolutionKind {
    /// The recorded element was found at the recorded point — the strongest
    /// semantic confirmation.
    RecordedPoint,
    /// The element had moved; the search spiral found it nearby.
    SpiralReresolved,
    /// The element was not found anywhere near the recorded point; replay
    /// fell back to the raw recorded coordinates.
    CoordinateFallback,
    /// No element descriptor was recorded — coordinates were the only
    /// available strategy.
    NoDescriptor,
}

/// One click's resolution outcome, collected into the run's step trace.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StepResolution {
    /// Index into the replayed event list.
    pub step_index: usize,
    pub kind: ResolutionKind,
    /// Recorded element name, when a descriptor existed.
    pub target_name: Option<String>,
    /// Screen point the click was actually sent to.
    pub point: (i32, i32),
}

/// Live per-step replay progress. Owned by the engine, advanced by the
/// platform replay loop just before each event executes, and polled by the
/// frontend (via `get_replay_progress`) to render per-step status. The step
/// counter is lock-free so the replay loop never blocks on a UI read; the
/// resolution trace takes a short Mutex only when a click resolves.
#[derive(Default)]
pub struct ReplayProgress {
    /// Index of the event currently (or last) being executed.
    current: AtomicUsize,
    /// Total number of events in the running replay.
    total: AtomicUsize,
    /// Per-click resolution outcomes for the running replay.
    trace: Mutex<Vec<StepResolution>>,
}

impl ReplayProgress {
    /// Reset for a new replay of `total` events.
    pub fn begin(&self, total: usize) {
        self.current.store(0, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
        self.trace.lock().unwrap().clear();
    }

    /// Mark event `idx` as the one now executing.
    pub fn set_step(&self, idx: usize) {
        self.current.store(idx, Ordering::Relaxed);
    }

    /// Snapshot as (current step index, total steps).
    pub fn snapshot(&self) -> (usize, usize) {
        (
            self.current.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    /// Record how a click at `step_index` resolved its target.
    pub fn record_resolution(&self, resolution: StepResolution) {
        self.trace.lock().unwrap().push(resolution);
    }

    /// Take the collected resolution trace, leaving it empty.
    pub fn take_trace(&self) -> Vec<StepResolution> {
        std::mem::take(&mut self.trace.lock().unwrap())
    }
}

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
    try_resolve_click_point_traced(target, rx, ry, lookup).map(|(point, _)| point)
}

/// Like `try_resolve_click_point`, but also reports *how* the target was
/// found: at the recorded point, or re-resolved nearby via the spiral. This
/// feeds the per-run resolution trace so fallback decisions are explainable.
pub fn try_resolve_click_point_traced<F>(
    target: &ElementInfo,
    rx: i32,
    ry: i32,
    lookup: F,
) -> Option<((i32, i32), ResolutionKind)>
where
    F: Fn(i32, i32) -> Option<ElementInfo>,
{
    if let Some(found) = lookup(rx, ry) {
        if descriptor_matches(target, &found) {
            return Some(((rx, ry), ResolutionKind::RecordedPoint));
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
                    return Some(((px, py), ResolutionKind::SpiralReresolved));
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

    // ── resolution tracing ────────────────────────────────────────────────

    #[test]
    fn traced_resolution_reports_recorded_point_vs_spiral() {
        let target = info("AXButton", "Save", "Notes");

        let at_point = target.clone();
        let traced = try_resolve_click_point_traced(&target, 10, 10, |x, y| {
            (x == 10 && y == 10).then(|| at_point.clone())
        });
        assert_eq!(traced, Some(((10, 10), ResolutionKind::RecordedPoint)));

        let moved = target.clone();
        let traced = try_resolve_click_point_traced(&target, 100, 100, |x, y| {
            (x == 170 && y == 100).then(|| moved.clone())
        });
        assert_eq!(traced, Some(((170, 100), ResolutionKind::SpiralReresolved)));

        let traced = try_resolve_click_point_traced(&target, 100, 100, |_, _| None);
        assert_eq!(traced, None);
    }

    #[test]
    fn progress_trace_collects_resolutions_and_clears_on_begin() {
        let progress = ReplayProgress::default();
        progress.begin(3);
        progress.record_resolution(StepResolution {
            step_index: 0,
            kind: ResolutionKind::RecordedPoint,
            target_name: Some("Save".into()),
            point: (10, 10),
        });
        progress.record_resolution(StepResolution {
            step_index: 2,
            kind: ResolutionKind::CoordinateFallback,
            target_name: Some("Send".into()),
            point: (50, 60),
        });

        let trace = progress.take_trace();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].kind, ResolutionKind::RecordedPoint);
        assert_eq!(trace[1].kind, ResolutionKind::CoordinateFallback);

        // take_trace drains…
        assert!(progress.take_trace().is_empty());

        // …and begin() clears any leftovers from an abandoned run.
        progress.record_resolution(StepResolution {
            step_index: 0,
            kind: ResolutionKind::NoDescriptor,
            target_name: None,
            point: (0, 0),
        });
        progress.begin(1);
        assert!(progress.take_trace().is_empty());
    }
}
