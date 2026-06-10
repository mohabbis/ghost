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
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workflow {
    pub name: String,
    pub events: Vec<InputEvent>,
    pub metadata: WorkflowMetadata,
    pub reliability: Option<ReliabilitySettings>,
}

impl Default for Workflow {
    fn default() -> Self {
        Workflow {
            name: String::new(),
            events: Vec::new(),
            metadata: WorkflowMetadata::default(),
            reliability: None,
        }
    }
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
