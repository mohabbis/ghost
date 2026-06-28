use crate::core::compression::compress;
use crate::core::compression::CompressionReport;
use crate::core::events::InputEvent;

/// Compress a raw event stream into semantic steps with warnings and metadata.
///
/// This deterministic read model turns InputEvent sequences into human-reviewable
/// CompressedSteps (Click, TypeText, Shortcut, Scroll, Wait, Unknown) plus a list
/// of warnings (coordinate-only targets, low confidence, secure-field typing).
///
/// Used by the frontend to show a review timeline before replay, and by Ghost Guard
/// to flag risky operations before execution.
#[tauri::command]
pub fn compress_workflow(events: Vec<InputEvent>) -> Result<CompressionReport, String> {
    if events.is_empty() {
        return Err("Workflow is empty".to_string());
    }

    let report = compress(&events);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::KeyAction;

    fn key_down(chars: &str) -> InputEvent {
        InputEvent::Key {
            code: 0,
            chars: chars.to_string(),
            modifiers: 0,
            action: KeyAction::Down,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
        }
    }

    fn key_up(chars: &str) -> InputEvent {
        InputEvent::Key {
            code: 0,
            chars: chars.to_string(),
            modifiers: 0,
            action: KeyAction::Up,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
        }
    }

    fn mouse_click(x: i32, y: i32) -> InputEvent {
        InputEvent::MouseClick {
            x,
            y,
            button: 0,
            element: None,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
            self_heal: None,
        }
    }

    #[test]
    fn compress_empty_workflow_fails() {
        let result = compress_workflow(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn compress_simple_click_and_type() {
        let events = vec![mouse_click(100, 200), key_down("a"), key_up("a")];
        let result = compress_workflow(events);
        assert!(result.is_ok());

        let report = result.unwrap();
        assert!(!report.steps.is_empty());
        assert_eq!(report.original_event_count, 3);
    }

    #[test]
    fn compression_report_shows_reduction() {
        let events = (0..10)
            .flat_map(|_| vec![key_down("x"), key_up("x")])
            .collect::<Vec<_>>();
        let result = compress_workflow(events);
        assert!(result.is_ok());

        let report = result.unwrap();
        assert!(report.reduction_ratio > 0.0);
        assert!(report.compressed_step_count < report.original_event_count);
    }
}
