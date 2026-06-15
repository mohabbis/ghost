use serde::{Deserialize, Serialize};

/// Retry configuration for a workflow step
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub backoff_ms: u64,
    pub backoff_multiplier: f32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        RetryConfig {
            max_attempts: 3,
            backoff_ms: 500,
            backoff_multiplier: 2.0,
        }
    }
}

/// Checkpoint for sensitive workflow steps
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Checkpoint {
    pub step_index: usize,
    pub prompt: String,
    pub requires_confirmation: bool,
}

/// Visual checkpoint for visual regression testing
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VisualCheckPoint {
    pub event_index: usize,
    pub name: String,
    pub baseline_screenshot_path: Option<String>,
    pub threshold: f32,
}

/// Reliability settings for workflow execution
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReliabilitySettings {
    pub retry_config: RetryConfig,
    pub checkpoints: Vec<Checkpoint>,
    pub continue_on_error: bool,
    pub validate_elements: bool,
}

impl Default for ReliabilitySettings {
    fn default() -> Self {
        ReliabilitySettings {
            retry_config: RetryConfig::default(),
            checkpoints: Vec::new(),
            continue_on_error: true,
            validate_elements: true,
        }
    }
}

/// Semantic tag for AI-enhanced understanding
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SemanticTag {
    pub action: String,  // "click", "type", "wait", "scroll"
    pub target: String,  // "Submit Button", "Username Field"
    pub confidence: f32, // 0.0 - 1.0
    pub ui_element: Option<ElementInfo>,
    pub ai_generated: bool,
}

/// Element selector for smart waiting
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ElementSelector {
    Coordinates {
        x: i32,
        y: i32,
    },
    Semantic {
        role: String,
        name: String,
        app: Option<String>,
    },
    OCR {
        text: String,
        fuzzy: bool,
    },
}

/// Wait condition types
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WaitCondition {
    ElementVisible { selector: ElementSelector },
    ElementExists { selector: ElementSelector },
    TextPresent { text: String },
    ImageMatches { baseline: String, threshold: f32 },
    Custom { js_expression: String },
}

/// Variable type for data-driven testing
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum VarType {
    RandomEmail,
    RandomString {
        length: usize,
    },
    Timestamp,
    FromCSV {
        path: String,
        column: String,
        row: Option<usize>,
    },
    FromEnv {
        key: String,
    },
}

/// Mismatch action for visual checks
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MismatchAction {
    Retry { attempts: u32 },
    Fail,
    LogOnly,
}

/// Shared event schema for cross-platform input events.
/// All variants are serializable for IPC transmission.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum InputEvent {
    MouseClick {
        x: i32,
        y: i32,
        button: u8,
        element: Option<ElementInfo>,
        timestamp: Option<u64>,
        retry_count: Option<u32>,
        semantic_tag: Option<SemanticTag>,
        self_heal: Option<bool>,
    },
    Key {
        code: u16,
        chars: String,
        modifiers: u8,
        action: KeyAction,
        timestamp: Option<u64>,
        retry_count: Option<u32>,
        semantic_tag: Option<SemanticTag>,
    },
    Scroll {
        dx: i32,
        dy: i32,
        phase: u8,
        timestamp: Option<u64>,
    },
    Delay {
        ms: u64,
        timestamp: Option<u64>,
    },
    /// Wait for a condition to be met
    Wait {
        condition: WaitCondition,
        timeout_ms: u64,
        poll_interval_ms: u64,
    },
    /// Visual regression check
    VisualCheck {
        baseline_screenshot: String,
        threshold: f32,
        on_mismatch: MismatchAction,
    },
    /// Variable for data-driven testing
    Variable {
        name: String,
        value_template: String,
        var_type: VarType,
    },
    /// Reference to a variable
    VariableRef {
        name: String,
    },
}

/// Metadata for AI-powered workflow analysis
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WorkflowMetadata {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub estimated_duration_ms: u64,
    pub reliability_score: f32,
    pub element_confidence: f32,
}

/// Enhanced workflow with metadata
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Workflow {
    pub name: String,
    pub events: Vec<InputEvent>,
    pub metadata: WorkflowMetadata,
    pub reliability: Option<ReliabilitySettings>,
}

/// Accessibility element metadata captured during recording.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ElementInfo {
    pub role: String,
    pub name: String,
    pub app: String,
    pub fallback_coords: Option<(i32, i32)>,
    /// Current element value (text field content, checkbox state, etc.)
    pub value: Option<String>,
    /// AXDescription / accessible description for unlabelled elements
    pub description: Option<String>,
    /// AXIdentifier / automation identifier — stable across runs
    pub identifier: Option<String>,
    /// Human-readable role description (e.g. "push button", "text field")
    pub role_description: Option<String>,
}

/// Keyboard action state for key events.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum KeyAction {
    Down,
    Up,
}

impl InputEvent {
    /// Recorded wall-clock timestamp (epoch ms), when the variant carries one.
    pub fn timestamp(&self) -> Option<u64> {
        match self {
            InputEvent::MouseClick { timestamp, .. }
            | InputEvent::Key { timestamp, .. }
            | InputEvent::Scroll { timestamp, .. }
            | InputEvent::Delay { timestamp, .. } => *timestamp,
            _ => None,
        }
    }

    /// Stamp the event with a wall-clock timestamp (epoch ms). Recording
    /// stamps events as they arrive so replay can reproduce the rhythm.
    pub fn set_timestamp(&mut self, ts_ms: u64) {
        match self {
            InputEvent::MouseClick { timestamp, .. }
            | InputEvent::Key { timestamp, .. }
            | InputEvent::Scroll { timestamp, .. }
            | InputEvent::Delay { timestamp, .. } => *timestamp = Some(ts_ms),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(event: &InputEvent) -> InputEvent {
        let json = serde_json::to_string(event).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn mouse_click_roundtrip() {
        let event = InputEvent::MouseClick {
            x: 10,
            y: 20,
            button: 0,
            element: Some(ElementInfo {
                role: "button".to_string(),
                name: "Submit".to_string(),
                app: "App".to_string(),
                ..Default::default()
            }),
            timestamp: Some(1234),
            retry_count: Some(1),
            semantic_tag: Some(SemanticTag {
                action: "click".to_string(),
                target: "Submit Button".to_string(),
                confidence: 0.9,
                ui_element: None,
                ai_generated: true,
            }),
            self_heal: Some(true),
        };

        let restored = roundtrip(&event);
        match restored {
            InputEvent::MouseClick {
                x,
                y,
                button,
                element,
                timestamp,
                retry_count,
                semantic_tag,
                self_heal,
            } => {
                assert_eq!(x, 10);
                assert_eq!(y, 20);
                assert_eq!(button, 0);
                assert_eq!(element.unwrap().name, "Submit");
                assert_eq!(timestamp, Some(1234));
                assert_eq!(retry_count, Some(1));
                assert_eq!(semantic_tag.unwrap().action, "click");
                assert_eq!(self_heal, Some(true));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn key_roundtrip() {
        let event = InputEvent::Key {
            code: 13,
            chars: "a".to_string(),
            modifiers: 1,
            action: KeyAction::Down,
            timestamp: Some(42),
            retry_count: None,
            semantic_tag: None,
        };

        let restored = roundtrip(&event);
        match restored {
            InputEvent::Key {
                code,
                chars,
                modifiers,
                action,
                timestamp,
                ..
            } => {
                assert_eq!(code, 13);
                assert_eq!(chars, "a");
                assert_eq!(modifiers, 1);
                assert!(matches!(action, KeyAction::Down));
                assert_eq!(timestamp, Some(42));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn scroll_roundtrip() {
        let event = InputEvent::Scroll {
            dx: 5,
            dy: -10,
            phase: 1,
            timestamp: Some(99),
        };

        let restored = roundtrip(&event);
        match restored {
            InputEvent::Scroll {
                dx,
                dy,
                phase,
                timestamp,
            } => {
                assert_eq!(dx, 5);
                assert_eq!(dy, -10);
                assert_eq!(phase, 1);
                assert_eq!(timestamp, Some(99));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn delay_roundtrip() {
        let event = InputEvent::Delay {
            ms: 500,
            timestamp: Some(1),
        };

        let restored = roundtrip(&event);
        match restored {
            InputEvent::Delay { ms, timestamp } => {
                assert_eq!(ms, 500);
                assert_eq!(timestamp, Some(1));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn wait_roundtrip() {
        let event = InputEvent::Wait {
            condition: WaitCondition::TextPresent {
                text: "Loading".to_string(),
            },
            timeout_ms: 5000,
            poll_interval_ms: 100,
        };

        let restored = roundtrip(&event);
        match restored {
            InputEvent::Wait {
                condition,
                timeout_ms,
                poll_interval_ms,
            } => {
                assert!(
                    matches!(condition, WaitCondition::TextPresent { text } if text == "Loading")
                );
                assert_eq!(timeout_ms, 5000);
                assert_eq!(poll_interval_ms, 100);
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn visual_check_roundtrip() {
        let event = InputEvent::VisualCheck {
            baseline_screenshot: "baseline.png".to_string(),
            threshold: 0.95,
            on_mismatch: MismatchAction::Retry { attempts: 3 },
        };

        let restored = roundtrip(&event);
        match restored {
            InputEvent::VisualCheck {
                baseline_screenshot,
                threshold,
                on_mismatch,
            } => {
                assert_eq!(baseline_screenshot, "baseline.png");
                assert_eq!(threshold, 0.95);
                assert!(matches!(on_mismatch, MismatchAction::Retry { attempts: 3 }));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn variable_roundtrip() {
        let event = InputEvent::Variable {
            name: "email".to_string(),
            value_template: "{{email}}".to_string(),
            var_type: VarType::RandomEmail,
        };

        let restored = roundtrip(&event);
        match restored {
            InputEvent::Variable {
                name,
                value_template,
                var_type,
            } => {
                assert_eq!(name, "email");
                assert_eq!(value_template, "{{email}}");
                assert!(matches!(var_type, VarType::RandomEmail));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn variable_ref_roundtrip() {
        let event = InputEvent::VariableRef {
            name: "email".to_string(),
        };

        let restored = roundtrip(&event);
        match restored {
            InputEvent::VariableRef { name } => assert_eq!(name, "email"),
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn timestamp_getter_and_setter() {
        let mut click = InputEvent::MouseClick {
            x: 0,
            y: 0,
            button: 0,
            element: None,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
            self_heal: None,
        };
        assert_eq!(click.timestamp(), None);
        click.set_timestamp(777);
        assert_eq!(click.timestamp(), Some(777));

        // Variants without a timestamp field are unaffected
        let mut var_ref = InputEvent::VariableRef {
            name: "x".to_string(),
        };
        assert_eq!(var_ref.timestamp(), None);
        var_ref.set_timestamp(123);
        assert_eq!(var_ref.timestamp(), None);
    }

    #[test]
    fn workflow_default_is_empty() {
        let workflow = Workflow::default();
        assert_eq!(workflow.name, "");
        assert!(workflow.events.is_empty());
        assert!(workflow.reliability.is_none());
        assert_eq!(workflow.metadata.name, WorkflowMetadata::default().name);
    }

    #[test]
    fn workflow_metadata_default_values() {
        let metadata = WorkflowMetadata::default();
        assert_eq!(metadata.name, "");
        assert_eq!(metadata.description, "");
        assert!(metadata.tags.is_empty());
        assert_eq!(metadata.created_at, 0);
        assert_eq!(metadata.updated_at, 0);
        assert_eq!(metadata.estimated_duration_ms, 0);
        assert_eq!(metadata.reliability_score, 0.0);
        assert_eq!(metadata.element_confidence, 0.0);
    }

    #[test]
    fn workflow_roundtrip_with_events() {
        let workflow = Workflow {
            name: "My Workflow".to_string(),
            events: vec![InputEvent::Delay {
                ms: 100,
                timestamp: None,
            }],
            metadata: WorkflowMetadata {
                name: "My Workflow".to_string(),
                ..Default::default()
            },
            reliability: Some(ReliabilitySettings::default()),
        };

        let json = serde_json::to_string(&workflow).unwrap();
        let restored: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "My Workflow");
        assert_eq!(restored.events.len(), 1);
        assert!(restored.reliability.is_some());
    }
}
