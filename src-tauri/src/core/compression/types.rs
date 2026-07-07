//! Data types for the deterministic event-compression pipeline.

use crate::core::events::ElementInfo;
use serde::{Deserialize, Serialize};

pub const LOW_CONFIDENCE: f32 = 0.5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CompressedStep {
    Click(ClickStep),
    TypeText(TypeTextStep),
    Shortcut(ShortcutStep),
    Scroll(ScrollStep),
    Wait(WaitStep),
    Unknown(UnknownStep),
}

impl CompressedStep {
    pub fn raw_event_count(&self) -> usize {
        match self {
            CompressedStep::Click(s) => s.raw_event_count,
            CompressedStep::TypeText(s) => s.raw_event_count,
            CompressedStep::Shortcut(s) => s.raw_event_count,
            CompressedStep::Scroll(s) => s.raw_event_count,
            CompressedStep::Wait(s) => s.raw_event_count,
            CompressedStep::Unknown(s) => s.raw_event_count,
        }
    }

    pub fn confidence(&self) -> f32 {
        match self {
            CompressedStep::Click(s) => s.confidence,
            CompressedStep::TypeText(s) => s.confidence,
            CompressedStep::Shortcut(_) | CompressedStep::Scroll(_) | CompressedStep::Wait(_) => {
                1.0
            }
            CompressedStep::Unknown(_) => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickButton {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClickStep {
    pub button: ClickButton,
    pub target: Option<Target>,
    pub fallback_coords: Option<(i32, i32)>,
    pub confidence: f32,
    pub raw_event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeTextStep {
    pub char_count: usize,
    pub redacted: bool,
    pub secure_field: bool,
    pub target: Option<Target>,
    pub text: Option<String>,
    pub confidence: f32,
    pub raw_event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortcutStep {
    pub combo: String,
    pub action: Option<String>,
    pub raw_event_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollMagnitude {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrollStep {
    pub direction: ScrollDirection,
    pub magnitude: ScrollMagnitude,
    pub raw_event_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitStep {
    pub ms: u64,
    pub raw_event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownStep {
    pub description: String,
    pub raw_event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub name: String,
    pub role: String,
    pub app: String,
    pub identifier: Option<String>,
}

impl Target {
    pub fn from_element(el: &ElementInfo) -> Self {
        Target {
            name: el.name.clone(),
            role: el.role.clone(),
            app: el.app.clone(),
            identifier: el.identifier.as_ref().filter(|s| !s.is_empty()).cloned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CompressionWarning {
    CoordinateOnlyTarget { step_index: usize },
    LowConfidence { step_index: usize, confidence: f32 },
    SecureFieldTyping { step_index: usize },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompressionReport {
    pub original_event_count: usize,
    pub compressed_step_count: usize,
    pub reduction_ratio: f32,
    pub redacted_fields: usize,
    pub warnings: Vec<CompressionWarning>,
    pub steps: Vec<CompressedStep>,
    /// Per-step `(raw_start, raw_len)` spans into the original event list,
    /// aligned index-for-index with `steps`. Spans are **not contiguous**:
    /// dropped sub-threshold delays and standalone mouse releases consume raw
    /// events without emitting a step, so never prefix-sum `raw_event_count`
    /// to reconstruct positions — use these spans (or
    /// [`CompressionReport::step_for_raw_index`]). `#[serde(default)]` so
    /// reports produced by older builds still deserialize.
    #[serde(default)]
    pub raw_spans: Vec<(usize, usize)>,
}

impl CompressionReport {
    /// Map a raw-event index (e.g. a replay trace's `StepResolution::step_index`)
    /// to the compressed step that consumed it. `None` when the index fell in
    /// a gap (dropped delay, standalone release) or spans are absent.
    pub fn step_for_raw_index(&self, raw_index: usize) -> Option<usize> {
        self.raw_spans
            .iter()
            .position(|&(start, len)| start <= raw_index && raw_index < start + len)
    }

    pub fn new(
        original_event_count: usize,
        steps: Vec<CompressedStep>,
        raw_spans: Vec<(usize, usize)>,
    ) -> Self {
        debug_assert_eq!(
            steps.len(),
            raw_spans.len(),
            "raw_spans must stay aligned with steps"
        );
        let compressed_step_count = steps.len();
        let reduction_ratio = if original_event_count == 0 {
            0.0
        } else {
            let saved = original_event_count.saturating_sub(compressed_step_count);
            saved as f32 / original_event_count as f32
        };

        let mut warnings = Vec::new();
        let mut redacted_fields = 0;
        for (idx, step) in steps.iter().enumerate() {
            match step {
                CompressedStep::Click(c) => {
                    if c.target.is_none() {
                        warnings.push(CompressionWarning::CoordinateOnlyTarget { step_index: idx });
                    }
                    if c.confidence <= LOW_CONFIDENCE {
                        warnings.push(CompressionWarning::LowConfidence {
                            step_index: idx,
                            confidence: c.confidence,
                        });
                    }
                }
                CompressedStep::TypeText(t) => {
                    if t.redacted {
                        redacted_fields += 1;
                    }
                    if t.secure_field {
                        warnings.push(CompressionWarning::SecureFieldTyping { step_index: idx });
                    }
                }
                _ => {}
            }
        }

        CompressionReport {
            original_event_count,
            compressed_step_count,
            reduction_ratio,
            redacted_fields,
            warnings,
            steps,
            raw_spans,
        }
    }
}
