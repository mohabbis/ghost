//! Deterministic-compression test suite.

use super::*;
use crate::core::events::{ElementInfo, InputEvent, KeyAction};

const CMD: u8 = 0x08;

fn press(button: u8, element: Option<ElementInfo>) -> InputEvent {
    InputEvent::MouseClick {
        x: 10,
        y: 20,
        button,
        element,
        timestamp: None,
        retry_count: None,
        semantic_tag: None,
        self_heal: None,
    }
}

fn left_press(element: Option<ElementInfo>) -> InputEvent {
    press(0, element)
}

fn left_release() -> InputEvent {
    press(1, None)
}

fn key(chars: &str, modifiers: u8, action: KeyAction) -> InputEvent {
    InputEvent::Key {
        code: 0,
        chars: chars.to_string(),
        modifiers,
        action,
        timestamp: None,
        retry_count: None,
        semantic_tag: None,
    }
}

fn typed(text: &str) -> Vec<InputEvent> {
    text.chars()
        .flat_map(|c| {
            vec![
                key(&c.to_string(), 0, KeyAction::Down),
                key(&c.to_string(), 0, KeyAction::Up),
            ]
        })
        .collect()
}

fn scroll(dx: i32, dy: i32) -> InputEvent {
    InputEvent::Scroll {
        dx,
        dy,
        phase: 0,
        timestamp: None,
    }
}

fn delay(ms: u64) -> InputEvent {
    InputEvent::Delay {
        ms,
        timestamp: None,
    }
}

fn element(role: &str, name: &str, app: &str, id: Option<&str>) -> ElementInfo {
    ElementInfo {
        role: role.to_string(),
        name: name.to_string(),
        app: app.to_string(),
        identifier: id.map(str::to_string),
        ..Default::default()
    }
}

#[test]
fn click_down_up_becomes_single_click() {
    let el = element("AXButton", "Export", "Chrome", Some("export-btn"));
    let report = compress(&[left_press(Some(el)), left_release()]);
    assert_eq!(report.steps.len(), 1);
    match &report.steps[0] {
        CompressedStep::Click(c) => {
            assert_eq!(c.button, ClickButton::Left);
            assert_eq!(c.raw_event_count, 2);
            assert_eq!(c.target.as_ref().unwrap().name, "Export");
        }
        other => panic!("expected click, got {other:?}"),
    }
}

#[test]
fn typing_sequence_becomes_redacted_type_step() {
    let report = compress(&typed("hello"));
    assert_eq!(report.steps.len(), 1);
    match &report.steps[0] {
        CompressedStep::TypeText(t) => {
            assert_eq!(t.char_count, 5);
            assert!(t.redacted);
            assert!(t.text.is_none());
        }
        other => panic!("expected type text, got {other:?}"),
    }
    assert_eq!(report.redacted_fields, 1);
}

#[test]
fn shortcut_is_recognized() {
    let report = compress(&[key("s", CMD, KeyAction::Down), key("s", CMD, KeyAction::Up)]);
    assert_eq!(report.steps.len(), 1);
    match &report.steps[0] {
        CompressedStep::Shortcut(s) => {
            assert_eq!(s.combo, "Cmd+S");
            assert_eq!(s.action.as_deref(), Some("Save"));
        }
        other => panic!("expected shortcut, got {other:?}"),
    }
}

#[test]
fn scroll_burst_is_merged() {
    let report = compress(&[scroll(0, 5), scroll(0, 6), scroll(0, 7)]);
    assert_eq!(report.steps.len(), 1);
    match &report.steps[0] {
        CompressedStep::Scroll(s) => {
            assert_eq!(s.direction, ScrollDirection::Down);
            assert_eq!(s.raw_event_count, 3);
        }
        other => panic!("expected scroll, got {other:?}"),
    }
}

#[test]
fn tiny_delays_are_removed() {
    let report = compress(&[delay(12), delay(30), delay(9)]);
    assert!(report.steps.is_empty());
    assert_eq!(report.original_event_count, 3);
}

#[test]
fn meaningful_delay_is_preserved() {
    let report = compress(&[delay(1500)]);
    assert_eq!(report.steps.len(), 1);
    match &report.steps[0] {
        CompressedStep::Wait(w) => assert_eq!(w.ms, 1500),
        other => panic!("expected wait, got {other:?}"),
    }
}

#[test]
fn coordinate_only_click_gets_low_confidence() {
    let report = compress(&[left_press(None), left_release()]);
    match &report.steps[0] {
        CompressedStep::Click(c) => {
            assert!(c.target.is_none());
            assert!(c.confidence <= LOW_CONFIDENCE);
            assert_eq!(c.fallback_coords, Some((10, 20)));
        }
        other => panic!("expected click, got {other:?}"),
    }
    assert!(report
        .warnings
        .iter()
        .any(|w| matches!(w, CompressionWarning::CoordinateOnlyTarget { .. })));
}

#[test]
fn secure_field_typing_is_redacted() {
    let secure = element("AXSecureTextField", "Password", "Safari", Some("pw"));
    let mut events = vec![left_press(Some(secure)), left_release()];
    events.extend(typed("hunter2"));
    let report = compress(&events);
    let type_step = report
        .steps
        .iter()
        .find_map(|s| match s {
            CompressedStep::TypeText(t) => Some(t),
            _ => None,
        })
        .expect("typing step");
    assert!(type_step.secure_field);
    assert!(type_step.redacted);
    assert!(type_step.text.is_none());
}

#[test]
fn opting_out_retains_nonsecure_text_but_never_secure() {
    let plain = element("AXTextField", "Search", "Chrome", Some("q"));
    let mut events = vec![left_press(Some(plain)), left_release()];
    events.extend(typed("cat"));
    let report = compress_with_options(&events, true);
    let t = report
        .steps
        .iter()
        .find_map(|s| match s {
            CompressedStep::TypeText(t) => Some(t),
            _ => None,
        })
        .unwrap();
    assert!(!t.redacted);
    assert_eq!(t.text.as_deref(), Some("cat"));

    let secure = element("AXSecureTextField", "Password", "Safari", None);
    let mut secure_events = vec![left_press(Some(secure)), left_release()];
    secure_events.extend(typed("cat"));
    let secure_report = compress_with_options(&secure_events, true);
    let secure_t = secure_report
        .steps
        .iter()
        .find_map(|s| match s {
            CompressedStep::TypeText(t) => Some(t),
            _ => None,
        })
        .unwrap();
    assert!(secure_t.redacted);
    assert!(secure_t.text.is_none());
}

#[test]
fn empty_stream_is_an_empty_report() {
    let report = compress(&[]);
    assert_eq!(report.original_event_count, 0);
    assert_eq!(report.compressed_step_count, 0);
    assert_eq!(report.reduction_ratio, 0.0);
    assert!(report.warnings.is_empty());
}

// ── raw-event spans (trace → semantic-step mapping) ─────────────────────────

#[test]
fn raw_spans_align_with_steps_and_cover_consumed_events() {
    // click(press+release) · short delay (dropped, no step) · type "hi" ·
    // scroll — spans must be aligned per step and start at the true raw index.
    let mut events = vec![left_press(None), left_release(), delay(50)];
    events.extend(typed("hi"));
    events.push(scroll(0, -30));

    let report = compress(&events);
    assert_eq!(report.raw_spans.len(), report.steps.len());
    // Click consumed events 0..2; the dropped delay (index 2) emits no step;
    // typing spans 3..7; scroll is index 7.
    assert_eq!(report.raw_spans[0], (0, 2));
    assert_eq!(report.raw_spans[1], (3, 4));
    assert_eq!(report.raw_spans[2], (7, 1));
}

#[test]
fn step_for_raw_index_maps_clicks_and_reports_gaps_as_none() {
    let mut events = vec![left_press(None), left_release(), delay(50)];
    events.extend(typed("hi"));

    let report = compress(&events);
    // Both halves of the click pair map to the Click step.
    assert_eq!(report.step_for_raw_index(0), Some(0));
    assert_eq!(report.step_for_raw_index(1), Some(0));
    // The dropped sub-threshold delay belongs to no step: a naive
    // prefix-sum of raw_event_count would misattribute it.
    assert_eq!(report.step_for_raw_index(2), None);
    assert_eq!(report.step_for_raw_index(3), Some(1));
    // Out of range.
    assert_eq!(report.step_for_raw_index(99), None);
}

#[test]
fn standalone_release_leaves_a_span_gap() {
    // A release with no preceding press (mid-recording start) is consumed
    // silently; the following click's span must still be exact.
    let events = vec![left_release(), left_press(None), left_release()];
    let report = compress(&events);
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.raw_spans[0], (1, 2));
    assert_eq!(report.step_for_raw_index(0), None);
    assert_eq!(report.step_for_raw_index(1), Some(0));
}

#[test]
fn meaningful_wait_gets_its_own_span() {
    let events = vec![
        left_press(None),
        left_release(),
        delay(1_000),
        scroll(0, 30),
    ];
    let report = compress(&events);
    assert_eq!(report.raw_spans, vec![(0, 2), (2, 1), (3, 1)]);
}
