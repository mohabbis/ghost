//! Headless fallback backend for platforms without a native automation API
//! (Linux dev machines, CI runners). Recording and element lookup are
//! unsupported — Ghost captures input via macOS CGEventTap or Windows hooks —
//! but replay synthesizes input through enigo when a display server is
//! available, and the crate compiles + the full test suite runs everywhere.

use crate::core::events::{ElementInfo, InputEvent, KeyAction, ReliabilitySettings};
use crate::core::replay_support::{
    check_continue, interruptible_sleep, pacing_gap_ms, ReplayProgress,
};
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct HeadlessBackend;

impl HeadlessBackend {
    pub fn recorder() -> Box<dyn InputRecorder> {
        Box::new(HeadlessRecorder)
    }

    pub fn locator() -> Box<dyn ElementLocator> {
        Box::new(HeadlessLocator)
    }

    pub fn replayer() -> Box<dyn ReplayEngine> {
        Box::new(HeadlessReplayer)
    }
}

struct HeadlessRecorder;

impl InputRecorder for HeadlessRecorder {
    fn start(&self, _tx: std::sync::mpsc::Sender<InputEvent>) -> anyhow::Result<()> {
        anyhow::bail!(
            "Recording is not supported on this platform. Ghost records input via \
             macOS CGEventTap or Windows hooks; this build supports replay only."
        )
    }

    fn stop(&self) {}
}

struct HeadlessLocator;

impl ElementLocator for HeadlessLocator {
    fn inspect_at(&self, _x: i32, _y: i32) -> anyhow::Result<Option<ElementInfo>> {
        // No accessibility tree to query; callers already treat None as
        // "no element here", so replay falls back to recorded coordinates.
        Ok(None)
    }
}

/// Map a recorded button code to the synthesized button + direction.
/// Recordings capture mouse-down (0/2) and mouse-up (1/3) as separate
/// events; replay mirrors press/release so single clicks don't double-fire
/// and drags / double-clicks survive faithfully.
fn click_action(button: u8) -> (Button, Direction) {
    match button {
        0 => (Button::Left, Direction::Press),
        1 => (Button::Left, Direction::Release),
        2 => (Button::Right, Direction::Press),
        3 => (Button::Right, Direction::Release),
        _ => (Button::Left, Direction::Click),
    }
}

struct HeadlessReplayer;

impl ReplayEngine for HeadlessReplayer {
    fn execute(
        &self,
        events: &[InputEvent],
        stop_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        speed: f32,
        progress: Arc<ReplayProgress>,
    ) -> anyhow::Result<()> {
        let mut enigo = Enigo::new(&Settings::default())?;
        let speed = speed.max(0.1);
        let mut prev_ts: Option<u64> = None;

        for (idx, event) in events.iter().enumerate() {
            progress.set_step(idx);
            if !check_continue(&stop_flag, &pause_flag) {
                return Ok(());
            }

            // Reproduce the recorded rhythm between events (recordings made
            // before timestamps existed simply run back-to-back).
            let gap = pacing_gap_ms(prev_ts, event.timestamp());
            if gap > 0 && !interruptible_sleep((gap as f32 / speed) as u64, &stop_flag, &pause_flag)
            {
                return Ok(());
            }
            if let Some(ts) = event.timestamp() {
                prev_ts = Some(ts);
            }

            match event {
                InputEvent::MouseClick { x, y, button, .. } => {
                    // No element re-resolution here: without an accessibility
                    // API the recorded coordinates are the best information.
                    let (mouse_button, direction) = click_action(*button);
                    enigo.move_mouse(*x, *y, Coordinate::Abs)?;
                    enigo.button(mouse_button, direction)?;
                }
                InputEvent::Key {
                    code,
                    chars,
                    action,
                    ..
                } => {
                    let key = if !chars.is_empty() {
                        Key::Unicode(chars.chars().next().unwrap_or(' '))
                    } else {
                        Key::Other(*code as u32)
                    };
                    match action {
                        KeyAction::Down => enigo.key(key, Direction::Press)?,
                        KeyAction::Up => enigo.key(key, Direction::Release)?,
                    }
                }
                InputEvent::Scroll { dx, dy, .. } => {
                    if *dx != 0 {
                        enigo.scroll(*dx, Axis::Horizontal)?;
                    }
                    if *dy != 0 {
                        enigo.scroll(*dy, Axis::Vertical)?;
                    }
                }
                InputEvent::Delay { ms, .. } => {
                    let adjusted_ms = (*ms as f32 / speed) as u64;
                    if !interruptible_sleep(adjusted_ms, &stop_flag, &pause_flag) {
                        return Ok(());
                    }
                }
                InputEvent::Wait {
                    condition,
                    timeout_ms,
                    poll_interval_ms,
                } => {
                    tracing::info!("Waiting for condition: {:?}", condition);
                    let locator = HeadlessBackend::locator();
                    let result = crate::core::wait::wait_for_condition(
                        condition,
                        locator.as_ref(),
                        *timeout_ms,
                        *poll_interval_ms,
                    );
                    match result {
                        crate::core::wait::WaitResult::Error(e) => {
                            tracing::warn!("Wait condition failed: {}", e);
                        }
                        crate::core::wait::WaitResult::Timeout => {
                            tracing::warn!("Wait condition timed out");
                        }
                        crate::core::wait::WaitResult::Success => {}
                    }
                }
                InputEvent::VisualCheck { .. } => {
                    // Screenshot capture is mac/windows-only (core::vision),
                    // so visual checks degrade to a logged skip here.
                    tracing::warn!("VisualCheck skipped: screenshots unsupported on this platform");
                }
                InputEvent::Variable {
                    name,
                    value_template,
                    var_type,
                } => {
                    let mut var_context = crate::core::wait::VariableContext::new();
                    let resolved = var_context
                        .resolve(name, var_type)
                        .unwrap_or_else(|_| value_template.clone());
                    var_context.set(name.clone(), resolved);
                }
                InputEvent::VariableRef { name } => {
                    tracing::debug!("Variable reference: {}", name);
                }
            }
            progress.complete_step(idx, event);
        }

        Ok(())
    }

    fn execute_with_reliability(
        &self,
        events: &[InputEvent],
        stop_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        speed: f32,
        progress: Arc<ReplayProgress>,
        _reliability: &ReliabilitySettings,
    ) -> anyhow::Result<()> {
        // Element validation and self-healing retries need an accessibility
        // API; without one the plain replay path is the honest behavior.
        self.execute(events, stop_flag, pause_flag, speed, progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `matches!` rather than `assert_eq!` because enigo's `Button`/`Direction`
    // don't guarantee `PartialEq`/`Debug` across versions.

    #[test]
    fn click_action_maps_button_down_to_press() {
        // Recorders emit mouse-down (0/2) as a Press so a single recorded click
        // synthesizes exactly one press — not a full Click that double-fires.
        assert!(matches!(click_action(0), (Button::Left, Direction::Press)));
        assert!(matches!(click_action(2), (Button::Right, Direction::Press)));
    }

    #[test]
    fn click_action_maps_button_up_to_release() {
        // Mouse-up (1/3) mirrors as Release so press/release pairs stay balanced
        // and drags / double-clicks survive replay faithfully.
        assert!(matches!(
            click_action(1),
            (Button::Left, Direction::Release)
        ));
        assert!(matches!(
            click_action(3),
            (Button::Right, Direction::Release)
        ));
    }

    #[test]
    fn click_action_unknown_button_falls_back_to_click() {
        // Defensive default for malformed recordings: a self-contained Click.
        assert!(matches!(click_action(9), (Button::Left, Direction::Click)));
    }

    #[test]
    fn recorder_start_is_unsupported() {
        // This backend records nothing; callers must get a clear error rather
        // than a silently no-op recording session.
        let (tx, _rx) = std::sync::mpsc::channel();
        let err = HeadlessRecorder.start(tx).unwrap_err();
        assert!(err.to_string().contains("not supported"));
    }

    #[test]
    fn locator_inspect_returns_none() {
        // No accessibility tree to query: callers treat None as "no element
        // here" and replay falls back to recorded coordinates.
        let result = HeadlessLocator.inspect_at(10, 20).unwrap();
        assert!(result.is_none());
    }
}
