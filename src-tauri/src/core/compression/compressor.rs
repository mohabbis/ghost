//! The compressor: fold a raw [`InputEvent`] stream into a [`CompressionReport`].

use crate::core::events::{ElementInfo, InputEvent, KeyAction};

use super::confidence;
use super::grouping::{
    MIN_MEANINGFUL_WAIT_MS, MouseKind, combo_label, is_shortcut, is_typed_char, known_action,
    mouse_kind, scroll_bucket,
};
use super::redaction::{REDACT_BY_DEFAULT, is_secure_target, resolve};
use super::types::*;

pub fn compress(events: &[InputEvent]) -> CompressionReport {
    compress_with_options(events, !REDACT_BY_DEFAULT)
}

pub fn compress_with_options(events: &[InputEvent], keep_text: bool) -> CompressionReport {
    let mut steps: Vec<CompressedStep> = Vec::new();
    // (raw_start, raw_len) per step, aligned with `steps`. Gaps are real:
    // dropped short delays and standalone releases consume events without
    // emitting a step.
    let mut raw_spans: Vec<(usize, usize)> = Vec::new();
    let mut current_focus: Option<ElementInfo> = None;

    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            InputEvent::MouseClick {
                x,
                y,
                button,
                element,
                ..
            } => {
                match mouse_kind(*button) {
                    Some(MouseKind::Press(btn)) => {
                        if let Some(el) = element {
                            current_focus = Some(el.clone());
                        }
                        let target = element.as_ref().map(Target::from_element);
                        let raw_event_count = if next_is_release(events, i) { 2 } else { 1 };
                        raw_spans.push((i, raw_event_count));
                        steps.push(CompressedStep::Click(ClickStep {
                            button: btn,
                            target,
                            fallback_coords: Some((*x, *y)),
                            confidence: confidence::score(element.as_ref()),
                            raw_event_count,
                        }));
                        if raw_event_count == 2 {
                            i += 1;
                        }
                    }
                    Some(MouseKind::Release(_)) => {}
                    None => {
                        raw_spans.push((i, 1));
                        steps.push(CompressedStep::Unknown(UnknownStep {
                            description: format!("Unrecognized mouse event (button {button})"),
                            raw_event_count: 1,
                        }));
                    }
                }
                i += 1;
            }
            InputEvent::Key {
                chars,
                modifiers,
                action,
                ..
            } => {
                if !matches!(action, KeyAction::Down) {
                    i += 1;
                    continue;
                }
                if is_typed_char(chars, *modifiers) {
                    let (count, consumed) = consume_typing_run(events, i);
                    let secure = is_secure_target(current_focus.as_ref());
                    let typed = collect_typed_text(events, i, consumed);
                    let (redacted, text) = resolve(&typed, secure, keep_text);
                    raw_spans.push((i, consumed));
                    steps.push(CompressedStep::TypeText(TypeTextStep {
                        char_count: count,
                        redacted,
                        secure_field: secure,
                        target: current_focus.as_ref().map(Target::from_element),
                        text,
                        confidence: confidence::score(current_focus.as_ref()),
                        raw_event_count: consumed,
                    }));
                    i += consumed;
                } else if is_shortcut(chars, *modifiers) {
                    let consumed = 1 + usize::from(next_is_key_up(events, i));
                    raw_spans.push((i, consumed));
                    steps.push(CompressedStep::Shortcut(ShortcutStep {
                        combo: combo_label(chars, *modifiers),
                        action: known_action(chars, *modifiers).map(str::to_string),
                        raw_event_count: consumed,
                    }));
                    i += consumed;
                } else {
                    let consumed = 1 + usize::from(next_is_key_up(events, i));
                    raw_spans.push((i, consumed));
                    steps.push(CompressedStep::Unknown(UnknownStep {
                        description: describe_special_key(chars),
                        raw_event_count: consumed,
                    }));
                    i += consumed;
                }
            }
            InputEvent::Scroll { .. } => {
                let (dx, dy, consumed) = consume_scroll_burst(events, i);
                let (direction, magnitude) = scroll_bucket(dx, dy);
                raw_spans.push((i, consumed));
                steps.push(CompressedStep::Scroll(ScrollStep {
                    direction,
                    magnitude,
                    raw_event_count: consumed,
                }));
                i += consumed;
            }
            InputEvent::Delay { .. } => {
                let (total_ms, consumed) = consume_delay_run(events, i);
                if total_ms >= MIN_MEANINGFUL_WAIT_MS {
                    raw_spans.push((i, consumed));
                    steps.push(CompressedStep::Wait(WaitStep {
                        ms: total_ms,
                        raw_event_count: consumed,
                    }));
                }
                i += consumed;
            }
            other => {
                raw_spans.push((i, 1));
                steps.push(CompressedStep::Unknown(UnknownStep {
                    description: describe_other(other),
                    raw_event_count: 1,
                }));
                i += 1;
            }
        }
    }

    CompressionReport::new(events.len(), steps, raw_spans)
}

fn next_is_release(events: &[InputEvent], i: usize) -> bool {
    matches!(
        events.get(i + 1),
        Some(InputEvent::MouseClick { button, .. }) if matches!(mouse_kind(*button), Some(MouseKind::Release(_)))
    )
}

fn next_is_key_up(events: &[InputEvent], i: usize) -> bool {
    matches!(
        events.get(i + 1),
        Some(InputEvent::Key {
            action: KeyAction::Up,
            ..
        })
    )
}

fn consume_typing_run(events: &[InputEvent], i: usize) -> (usize, usize) {
    let mut count = 0;
    let mut consumed = 0;
    let mut j = i;
    while let Some(InputEvent::Key {
        chars,
        modifiers,
        action,
        ..
    }) = events.get(j)
    {
        match action {
            KeyAction::Down if is_typed_char(chars, *modifiers) => {
                count += 1;
                consumed += 1;
            }
            KeyAction::Up => consumed += 1,
            _ => break,
        }
        j += 1;
    }
    (count, consumed)
}

fn collect_typed_text(events: &[InputEvent], i: usize, consumed: usize) -> String {
    let mut out = String::new();
    for ev in &events[i..i + consumed] {
        if let InputEvent::Key {
            chars,
            modifiers,
            action: KeyAction::Down,
            ..
        } = ev
            && is_typed_char(chars, *modifiers)
        {
            out.push_str(chars);
        }
    }
    out
}

fn consume_scroll_burst(events: &[InputEvent], i: usize) -> (i64, i64, usize) {
    let mut dx = 0i64;
    let mut dy = 0i64;
    let mut consumed = 0;
    let mut j = i;
    while let Some(InputEvent::Scroll { dx: ex, dy: ey, .. }) = events.get(j) {
        dx += *ex as i64;
        dy += *ey as i64;
        consumed += 1;
        j += 1;
    }
    (dx, dy, consumed)
}

fn consume_delay_run(events: &[InputEvent], i: usize) -> (u64, usize) {
    let mut total = 0u64;
    let mut consumed = 0;
    let mut j = i;
    while let Some(InputEvent::Delay { ms, .. }) = events.get(j) {
        total = total.saturating_add(*ms);
        consumed += 1;
        j += 1;
    }
    (total, consumed)
}

fn describe_special_key(chars: &str) -> String {
    let label = match chars {
        "\r" | "\n" => "Return",
        "\t" => "Tab",
        "\u{1b}" => "Escape",
        "\u{8}" | "\u{7f}" => "Delete",
        " " => "Space",
        other if !other.is_empty() => return format!("Press key '{}'", other.escape_default()),
        _ => "key",
    };
    format!("Press {label}")
}

fn describe_other(event: &InputEvent) -> String {
    match event {
        InputEvent::Wait { .. } => "Wait for a condition".to_string(),
        InputEvent::VisualCheck { .. } => "Visual regression check".to_string(),
        InputEvent::Variable { name, .. } => format!("Define variable '{name}'"),
        InputEvent::VariableRef { name } => format!("Use variable '{name}'"),
        _ => "Unrecognized event".to_string(),
    }
}
