//! Shared replay plumbing used by both platform replay engines:
//! pause/cancel-aware control flow, timestamp-based pacing, and element
//! re-resolution (self-healing) helpers.
//!
//! Everything here is platform-agnostic and unit-tested; the platform
//! modules supply only the raw "what element is at (x, y)" lookup.

use crate::audit::replay_undo_journal::{ReplayRunReport, ReplayUndoJournal};
use crate::core::events::{ElementInfo, InputEvent};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    /// The element's window had moved; it was found at the recorded
    /// window-relative offset inside the window's current frame.
    WindowRelative,
    /// The element had moved; the search spiral found it nearby.
    SpiralReresolved,
    /// The element name had slightly drifted or changed, but was resolved fuzzy-matching.
    FuzzyReresolved,
    /// No accessibility descriptor matched, but a screenshot crop taken at
    /// record time (`ElementInfo::template_png`) was found nearby via pixel
    /// template matching (`core::template_match`).
    TemplateMatched,
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
    /// Write-ahead replay run journal + persistence hook.
    wal: Mutex<Option<ReplayWalState>>,
}

struct ReplayWalState {
    journal: ReplayUndoJournal,
    events_total: usize,
    on_update: Box<dyn Fn(&ReplayRunReport) + Send + Sync>,
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

    /// Attach a write-ahead journal that persists after every completed event.
    pub fn begin_wal<F>(&self, events_total: usize, on_update: F)
    where
        F: Fn(&ReplayRunReport) + Send + Sync + 'static,
    {
        *self.wal.lock().unwrap() = Some(ReplayWalState {
            journal: ReplayUndoJournal::new(),
            events_total,
            on_update: Box::new(on_update),
        });
    }

    /// Clear any attached WAL hook (after replay finishes or aborts).
    pub fn clear_wal(&self) {
        *self.wal.lock().unwrap() = None;
    }

    /// Record a successfully replayed event into the WAL journal.
    pub fn complete_step(&self, step_index: usize, event: &InputEvent) {
        if let Some(state) = self.wal.lock().unwrap().as_mut() {
            state.journal.record_event(step_index, event);
            let report = ReplayRunReport {
                events_applied: step_index + 1,
                events_total: state.events_total,
                undo: state.journal.clone(),
            };
            (state.on_update)(&report);
        }
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
    if let (Some(t_id), Some(f_id)) = (&target.identifier, &found.identifier)
        && !t_id.is_empty()
    {
        return t_id == f_id;
    }
    if target.role.is_empty() || !target.role.eq_ignore_ascii_case(&found.role) {
        return false;
    }
    if !target.name.is_empty() {
        return target.name.eq_ignore_ascii_case(&found.name);
    }
    if target.app.is_empty() || target.app == "Unknown" {
        // Nameless target with no usable app: when BOTH sides carry a window
        // title, the title discriminates same-role elements in different
        // windows. When either side lacks one (recordings made before window
        // capture existed), keep the permissive role-only match so old
        // workflows behave exactly as before.
        match (&target.window_title, &found.window_title) {
            (Some(t), Some(f)) if !t.is_empty() && !f.is_empty() => t.eq_ignore_ascii_case(f),
            _ => true,
        }
    } else {
        target.app.eq_ignore_ascii_case(&found.app)
    }
}

/// Does the live element `found` match the recorded `target` descriptor fuzzy-matching
/// (when exact name match has failed)?
pub fn descriptor_matches_fuzzy(target: &ElementInfo, found: &ElementInfo) -> bool {
    // Role must still match case-insensitively.
    if target.role.is_empty() || !target.role.eq_ignore_ascii_case(&found.role) {
        return false;
    }

    // Both must have non-empty names.
    if target.name.is_empty() || found.name.is_empty() {
        return false;
    }

    // App must match case-insensitively if we have one.
    if !target.app.is_empty()
        && target.app != "Unknown"
        && !target.app.eq_ignore_ascii_case(&found.app)
    {
        return false;
    }

    let t_name = target.name.to_lowercase();
    let f_name = found.name.to_lowercase();

    // Check if either is a substring of the other (minimum length 3 to avoid matching noise).
    if t_name.len() >= 3
        && f_name.len() >= 3
        && (t_name.contains(&f_name) || f_name.contains(&t_name))
    {
        return true;
    }

    false
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
pub fn try_resolve_click_point<F, W, S>(
    target: &ElementInfo,
    rx: i32,
    ry: i32,
    lookup: F,
    window_origin: W,
    screenshot: S,
) -> Option<(i32, i32)>
where
    F: Fn(i32, i32) -> Option<ElementInfo>,
    W: Fn(&ElementInfo) -> Option<(i32, i32)>,
    S: Fn() -> Option<image::DynamicImage>,
{
    try_resolve_click_point_traced(target, rx, ry, lookup, window_origin, screenshot)
        .map(|(point, _)| point)
}

/// Like `try_resolve_click_point`, but also reports *how* the target was
/// found. Strategy order (strongest signal first, then cheapest):
///
/// 1. recorded point — element still where it was captured;
/// 2. window-relative — `window_origin` locates the recorded window's
///    current frame, and the element is verified at the recorded offset
///    inside it (one lookup; survives arbitrarily large window moves);
/// 3. spiral — scan outward around the recorded point (element moved
///    within its window, or window moved slightly);
/// 4. template match — when a screenshot crop was captured at record time
///    (`ElementInfo::template_png`), search a bounded region around the
///    recorded point for those pixels (`core::template_match`). Pixel-level,
///    so it can succeed where every accessibility-tree strategy above it
///    failed (descriptor changed, or none was ever recorded).
/// 5. `None` — callers decide whether to coordinate-fall-back or retry.
///
/// `window_origin` receives the full recorded element (platforms use
/// `window_title`, and on macOS also `app` to locate the owning process) and
/// returns the window's current top-left corner; platforms without a window
/// lookup pass `|_| None`, which skips strategy 2 entirely. `screenshot`
/// captures the current screen on demand; it's only called (once) if
/// strategies 1-3 all fail and a template is present, so platforms that
/// can't cheaply screenshot can pass `|| None` to skip strategy 4 entirely.
pub fn try_resolve_click_point_traced<F, W, S>(
    target: &ElementInfo,
    rx: i32,
    ry: i32,
    lookup: F,
    window_origin: W,
    screenshot: S,
) -> Option<((i32, i32), ResolutionKind)>
where
    F: Fn(i32, i32) -> Option<ElementInfo>,
    W: Fn(&ElementInfo) -> Option<(i32, i32)>,
    S: Fn() -> Option<image::DynamicImage>,
{
    // Pass 1: Exact matches (avoiding decoy false-positives first)
    if let Some(found) = lookup(rx, ry)
        && descriptor_matches(target, &found)
    {
        return Some(((rx, ry), ResolutionKind::RecordedPoint));
    }

    if let (Some(_), Some((relx, rely))) = (target.window_title.as_deref(), target.window_rel)
        && let Some((ox, oy)) = window_origin(target)
    {
        let (px, py) = (ox + relx, oy + rely);
        if (px, py) != (rx, ry)
            && px >= 0
            && py >= 0
            && let Some(found) = lookup(px, py)
            && descriptor_matches(target, &found)
        {
            return Some(((px, py), ResolutionKind::WindowRelative));
        }
    }

    for r in SEARCH_RADII {
        for (dx, dy) in SEARCH_DIRS {
            let (px, py) = (rx + dx * r, ry + dy * r);
            if px < 0 || py < 0 {
                continue;
            }
            if let Some(found) = lookup(px, py)
                && descriptor_matches(target, &found)
            {
                return Some(((px, py), ResolutionKind::SpiralReresolved));
            }
        }
    }

    // Pass 2: Fuzzy matches (only when exact match couldn't be found)
    if let Some(found) = lookup(rx, ry)
        && descriptor_matches_fuzzy(target, &found)
    {
        return Some(((rx, ry), ResolutionKind::FuzzyReresolved));
    }

    if let (Some(_), Some((relx, rely))) = (target.window_title.as_deref(), target.window_rel)
        && let Some((ox, oy)) = window_origin(target)
    {
        let (px, py) = (ox + relx, oy + rely);
        if (px, py) != (rx, ry)
            && px >= 0
            && py >= 0
            && let Some(found) = lookup(px, py)
            && descriptor_matches_fuzzy(target, &found)
        {
            return Some(((px, py), ResolutionKind::FuzzyReresolved));
        }
    }

    for r in SEARCH_RADII {
        for (dx, dy) in SEARCH_DIRS {
            let (px, py) = (rx + dx * r, ry + dy * r);
            if px < 0 || py < 0 {
                continue;
            }
            if let Some(found) = lookup(px, py)
                && descriptor_matches_fuzzy(target, &found)
            {
                return Some(((px, py), ResolutionKind::FuzzyReresolved));
            }
        }
    }

    // Pass 3: pixel template match. Only reachable when every accessibility-
    // tree strategy above has already failed and a template was actually
    // captured; `screenshot()` is a no-op closure (`|| None`) for callers
    // that skip this strategy, so it costs nothing when unused.
    if let Some(template_bytes) = &target.template_png
        && let Some(point) = resolve_via_template(template_bytes, rx, ry, &screenshot)
    {
        return Some((point, ResolutionKind::TemplateMatched));
    }

    None
}

/// Search a bounded region around the recorded point for `template_bytes`
/// (see [`crate::core::template_match`]). Bounded the same way the spiral
/// scan is (`SEARCH_RADII`'s max radius) rather than searching the whole
/// screen — cheaper, and a UI element that moved further than that is
/// unlikely to be the same control the user meant to click.
fn resolve_via_template<S>(
    template_bytes: &[u8],
    rx: i32,
    ry: i32,
    screenshot: &S,
) -> Option<(i32, i32)>
where
    S: Fn() -> Option<image::DynamicImage>,
{
    use image::GenericImageView;

    let template = image::load_from_memory(template_bytes).ok()?;
    let screen = screenshot()?;

    let margin = *SEARCH_RADII.last().expect("SEARCH_RADII is non-empty");
    let (screen_w, screen_h) = screen.dimensions();
    let x0 = (rx - margin).max(0) as u32;
    let y0 = (ry - margin).max(0) as u32;
    let x1 = ((rx + margin).max(0) as u32).min(screen_w);
    let y1 = ((ry + margin).max(0) as u32).min(screen_h);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let region = screen.crop_imm(x0, y0, x1 - x0, y1 - y0);

    let m = crate::core::template_match::find_template(&region, &template)?;
    if m.score < crate::core::template_match::DEFAULT_MIN_SCORE {
        return None;
    }
    // Click the matched region's center, not its top-left corner.
    let (tw, th) = template.dimensions();
    Some((
        x0 as i32 + m.x as i32 + (tw / 2) as i32,
        y0 as i32 + m.y as i32 + (th / 2) as i32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

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
        let resolved = try_resolve_click_point(
            &target,
            10,
            10,
            |x, y| (x == 10 && y == 10).then(|| at_point.clone()),
            |_| None,
            || None,
        );
        assert_eq!(resolved, Some((10, 10)));
    }

    #[test]
    fn finds_moved_element_nearby() {
        let target = info("AXButton", "Save", "Notes");
        let moved = target.clone();
        // Element now lives 70px to the right of where it was recorded.
        let resolved = try_resolve_click_point(
            &target,
            100,
            100,
            |x, y| (x == 170 && y == 100).then(|| moved.clone()),
            |_| None,
            || None,
        );
        assert_eq!(resolved, Some((170, 100)));
    }

    #[test]
    fn returns_none_when_element_gone() {
        let target = info("AXButton", "Save", "Notes");
        let resolved = try_resolve_click_point(&target, 100, 100, |_, _| None, |_| None, || None);
        assert_eq!(resolved, None);
    }

    // ── window-title matching ─────────────────────────────────────────────

    #[test]
    fn nameless_target_titles_discriminate_when_both_present() {
        let mut target = info("AXButton", "", "Unknown");
        target.window_title = Some("Invoices".into());
        let mut same_window = info("AXButton", "anything", "AnyApp");
        same_window.window_title = Some("invoices".into());
        let mut other_window = same_window.clone();
        other_window.window_title = Some("Chat".into());

        assert!(descriptor_matches(&target, &same_window));
        assert!(!descriptor_matches(&target, &other_window));
    }

    #[test]
    fn nameless_target_without_titles_keeps_permissive_match() {
        // Old recordings carry no window titles: behavior must be identical
        // to before title-aware matching existed.
        let target = info("AXButton", "", "Unknown");
        let mut found = info("AXButton", "anything", "AnyApp");
        assert!(descriptor_matches(&target, &found));
        // One-sided titles also stay permissive.
        found.window_title = Some("Chat".into());
        assert!(descriptor_matches(&target, &found));
    }

    #[test]
    fn named_target_ignores_title_drift() {
        // Titles often carry document names that legitimately change; a
        // role+name match must not be rejected because titles differ.
        let mut target = info("AXButton", "Save", "Notes");
        target.window_title = Some("Report v1".into());
        let mut found = info("AXButton", "Save", "Notes");
        found.window_title = Some("Report v2".into());
        assert!(descriptor_matches(&target, &found));
    }

    // ── window-relative resolution ────────────────────────────────────────

    fn windowed_target() -> ElementInfo {
        let mut t = info("AXButton", "Save", "Notes");
        t.window_title = Some("Report".into());
        t.window_rel = Some((40, 30));
        t
    }

    #[test]
    fn window_relative_resolves_far_window_move() {
        // Recorded at (140, 130) in a window at (100, 100); the window is now
        // at (900, 500) — far beyond spiral range. The window lookup plus the
        // recorded offset finds it in one step.
        let target = windowed_target();
        let moved = target.clone();
        let traced = try_resolve_click_point_traced(
            &target,
            140,
            130,
            |x, y| (x == 940 && y == 530).then(|| moved.clone()),
            |el| (el.window_title.as_deref() == Some("Report")).then_some((900, 500)),
            || None,
        );
        assert_eq!(traced, Some(((940, 530), ResolutionKind::WindowRelative)));
    }

    #[test]
    fn window_relative_candidate_is_verified_not_blind_clicked() {
        // The window is found, but the element at the recorded offset no
        // longer matches (window contents rearranged) — the chain must fall
        // through to the spiral / None rather than click the wrong thing.
        let target = windowed_target();
        let stranger = info("AXTextField", "Search", "Notes");
        let traced = try_resolve_click_point_traced(
            &target,
            140,
            130,
            |x, y| (x == 940 && y == 530).then(|| stranger.clone()),
            |el| (el.window_title.as_deref() == Some("Report")).then_some((900, 500)),
            || None,
        );
        assert_eq!(traced, None);
    }

    #[test]
    fn recorded_point_wins_over_window_relative() {
        // If the element still matches at the recorded point, that's the
        // answer — even when the window lookup would also succeed.
        let target = windowed_target();
        let at_point = target.clone();
        let traced = try_resolve_click_point_traced(
            &target,
            140,
            130,
            |x, y| (x == 140 && y == 130).then(|| at_point.clone()),
            |_| Some((900, 500)),
            || None,
        );
        assert_eq!(traced, Some(((140, 130), ResolutionKind::RecordedPoint)));
    }

    // ── resolution tracing ────────────────────────────────────────────────

    #[test]
    fn traced_resolution_reports_recorded_point_vs_spiral() {
        let target = info("AXButton", "Save", "Notes");

        let at_point = target.clone();
        let traced = try_resolve_click_point_traced(
            &target,
            10,
            10,
            |x, y| (x == 10 && y == 10).then(|| at_point.clone()),
            |_| None,
            || None,
        );
        assert_eq!(traced, Some(((10, 10), ResolutionKind::RecordedPoint)));

        let moved = target.clone();
        let traced = try_resolve_click_point_traced(
            &target,
            100,
            100,
            |x, y| (x == 170 && y == 100).then(|| moved.clone()),
            |_| None,
            || None,
        );
        assert_eq!(traced, Some(((170, 100), ResolutionKind::SpiralReresolved)));

        let traced =
            try_resolve_click_point_traced(&target, 100, 100, |_, _| None, |_| None, || None);
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
