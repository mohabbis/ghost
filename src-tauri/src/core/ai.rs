//! AI-powered workflow analysis and optimization.
//! Provides intelligent insights from recorded user workflows.

use crate::core::events::{InputEvent, WorkflowMetadata};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pattern detected in workflow
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DetectedPattern {
    pub pattern_type: PatternType,
    pub description: String,
    pub occurrences: Vec<usize>,
    pub confidence: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PatternType {
    RepetitiveClick,
    FormFill,
    Navigation,
    DataEntry,
}

/// AI analysis result for a workflow
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WorkflowAnalysis {
    pub workflow_name: String,
    pub total_events: usize,
    pub estimated_duration_ms: u64,
    pub patterns: Vec<DetectedPattern>,
    pub suggested_optimizations: Vec<OptimizationSuggestion>,
    pub reliability_score: f32,
    pub element_richness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OptimizationSuggestion {
    pub suggestion_type: String,
    pub description: String,
    pub impact_score: f32,
    pub affected_events: Vec<usize>,
}

/// Engine for analyzing workflows and providing AI-powered suggestions
pub struct WorkflowAnalyzer;

impl WorkflowAnalyzer {
    pub fn new() -> Self {
        WorkflowAnalyzer
    }

    /// Analyze a workflow and return insights
    pub fn analyze(&self, events: &[InputEvent], metadata: &WorkflowMetadata) -> WorkflowAnalysis {
        let total_events = events.len();
        let mut patterns = Vec::new();
        let mut optimizations = Vec::new();

        // Detect repetitive patterns
        let repetitive = self.detect_repetitive_patterns(events);
        if !repetitive.is_empty() {
            patterns.extend(repetitive);
        }

        // Detect form filling patterns
        let forms = self.detect_form_patterns(events);
        if !forms.is_empty() {
            patterns.extend(forms);
        }

        // Calculate reliability score
        let reliability = self.calculate_reliability(events);

        // Calculate element richness
        let element_richness = self.calculate_element_richness(events);

        // Generate optimization suggestions
        self.generate_optimizations(events, &patterns, &mut optimizations);

        WorkflowAnalysis {
            workflow_name: metadata.name.clone(),
            total_events,
            estimated_duration_ms: metadata.estimated_duration_ms,
            patterns,
            suggested_optimizations: optimizations,
            reliability_score: reliability,
            element_richness,
        }
    }

    /// Detect repetitive click patterns
    fn detect_repetitive_patterns(&self, events: &[InputEvent]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();
        let mut click_positions: HashMap<(i32, i32, u8), Vec<usize>> = HashMap::new();

        for (idx, event) in events.iter().enumerate() {
            if let InputEvent::MouseClick { x, y, button, .. } = event {
                click_positions
                    .entry((*x, *y, *button))
                    .or_default()
                    .push(idx);
            }
        }

        for (pos, occurrences) in click_positions {
            if occurrences.len() >= 2 {
                patterns.push(DetectedPattern {
                    pattern_type: PatternType::RepetitiveClick,
                    description: format!(
                        "Click at ({}, {}) repeated {} times",
                        pos.0,
                        pos.1,
                        occurrences.len()
                    ),
                    occurrences: occurrences.clone(),
                    confidence: (occurrences.len() as f32 / events.len() as f32).min(1.0),
                });
            }
        }

        patterns
    }

    /// Detect form filling patterns
    fn detect_form_patterns(&self, events: &[InputEvent]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();
        let mut key_sequences: Vec<usize> = Vec::new();

        for (idx, event) in events.iter().enumerate() {
            if let InputEvent::Key { action, .. } = event {
                if matches!(action, crate::core::events::KeyAction::Down) {
                    key_sequences.push(idx);
                }
            }
        }

        if key_sequences.len() >= 5 {
            patterns.push(DetectedPattern {
                pattern_type: PatternType::FormFill,
                description: format!(
                    "Form filling detected with {} keystrokes",
                    key_sequences.len()
                ),
                occurrences: key_sequences.clone(),
                confidence: (key_sequences.len() as f32 / events.len() as f32).min(1.0),
            });
        }

        patterns
    }

    /// Calculate workflow reliability score
    pub fn calculate_reliability(&self, events: &[InputEvent]) -> f32 {
        let mut score = 1.0;

        // Penalize for missing element info
        let missing_elements: usize = events
            .iter()
            .filter(|e| {
                if let InputEvent::MouseClick { element, .. } = e {
                    element.is_none()
                } else {
                    false
                }
            })
            .count();

        score -= missing_elements as f32 / events.len().max(1) as f32 * 0.3;

        // Penalize for long delays
        let long_delays: usize = events
            .iter()
            .filter(|e| {
                if let InputEvent::Delay { ms, .. } = e {
                    *ms > 5000
                } else {
                    false
                }
            })
            .count();

        score -= long_delays as f32 / events.len().max(1) as f32 * 0.2;

        score.max(0.0)
    }

    /// Calculate how "element-rich" the workflow is
    pub fn calculate_element_richness(&self, events: &[InputEvent]) -> f32 {
        let mut element_count = 0usize;

        for event in events {
            if let InputEvent::MouseClick { element, .. } = event {
                if element.is_some() {
                    element_count += 1;
                }
            }
        }

        element_count as f32 / events.len().max(1) as f32
    }

    /// Generate optimization suggestions
    fn generate_optimizations(
        &self,
        events: &[InputEvent],
        patterns: &[DetectedPattern],
        optimizations: &mut Vec<OptimizationSuggestion>,
    ) {
        for pattern in patterns {
            match pattern.pattern_type {
                PatternType::RepetitiveClick => {
                    optimizations.push(OptimizationSuggestion {
                        suggestion_type: "loop_extraction".to_string(),
                        description: format!("Extract repetitive click to a loop. Found {} occurrences of the same click pattern.", pattern.occurrences.len()),
                        impact_score: pattern.confidence,
                        affected_events: pattern.occurrences.clone(),
                    });
                }
                PatternType::FormFill => {
                    optimizations.push(OptimizationSuggestion {
                        suggestion_type: "form_handler".to_string(),
                        description: "Detected form filling pattern. Consider using a dedicated form handler with retry logic.".to_string(),
                        impact_score: 0.8,
                        affected_events: pattern.occurrences.clone(),
                    });
                }
                _ => {}
            }
        }

        // Check for optimization opportunities
        for (idx, event) in events.iter().enumerate() {
            if let InputEvent::Delay { ms, .. } = event {
                if *ms > 1000 {
                    optimizations.push(OptimizationSuggestion {
                        suggestion_type: "conditional_wait".to_string(),
                        description: format!(
                            "Replace {ms}ms delay with conditional wait for element state"
                        ),
                        impact_score: 0.7,
                        affected_events: vec![idx],
                    });
                }
            }
        }
    }

    /// Generate a named workflow with metadata from events
    pub fn generate_workflow_name(&self, events: &[InputEvent]) -> String {
        // Simple heuristic: look for common patterns
        let has_clicks = events
            .iter()
            .any(|e| matches!(e, InputEvent::MouseClick { .. }));
        let has_keys = events.iter().any(|e| matches!(e, InputEvent::Key { .. }));

        if has_clicks && has_keys {
            "Form Submission".to_string()
        } else if has_clicks {
            "Click Macro".to_string()
        } else if has_keys {
            "Keyboard Shortcut".to_string()
        } else {
            "Workflow".to_string()
        }
    }

    /// Suggest element improvements by analyzing the workflow
    pub fn suggest_element_improvements(&self, events: &[InputEvent]) -> Vec<ElementImprovement> {
        let mut improvements = Vec::new();

        for event in events {
            if let InputEvent::MouseClick { x, y, element, .. } = event {
                if element.is_none() {
                    improvements.push(ElementImprovement {
                        event_index: 0,
                        x: *x,
                        y: *y,
                        suggestion: "Element inspection recommended - consider verifying the target UI element".to_string(),
                    });
                }
            }
        }

        improvements
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ElementImprovement {
    pub event_index: usize,
    pub x: i32,
    pub y: i32,
    pub suggestion: String,
}

/// Workflow optimizer that applies AI-powered optimizations
pub struct WorkflowOptimizer;

impl WorkflowOptimizer {
    pub fn new() -> Self {
        WorkflowOptimizer
    }

    /// Optimize a workflow by applying various transformations
    pub fn optimize(&self, events: &[InputEvent]) -> anyhow::Result<Vec<InputEvent>> {
        let mut optimized = events.to_vec();

        // Merge consecutive delays
        optimized = self.merge_consecutive_delays(optimized);

        // Drop exact duplicate events (hook re-entry glitches)
        optimized = self.remove_duplicate_events(optimized);

        Ok(optimized)
    }

    /// Merge consecutive delay events into a single delay
    fn merge_consecutive_delays(&self, events: Vec<InputEvent>) -> Vec<InputEvent> {
        let mut result = Vec::new();
        let mut pending_delay: Option<u64> = None;

        for event in events {
            match &event {
                InputEvent::Delay { ms, .. } => {
                    if let Some(pd) = pending_delay.take() {
                        result.push(InputEvent::Delay {
                            ms: pd + ms,
                            timestamp: None,
                        });
                    } else {
                        pending_delay = Some(*ms);
                    }
                }
                _ => {
                    if let Some(pd) = pending_delay.take() {
                        result.push(InputEvent::Delay {
                            ms: pd,
                            timestamp: None,
                        });
                    }
                    result.push(event);
                }
            }
        }

        if let Some(pd) = pending_delay {
            result.push(InputEvent::Delay {
                ms: pd,
                timestamp: None,
            });
        }

        result
    }

    /// Drop *exact* duplicate consecutive events — same serialized payload,
    /// including the recorded timestamp. These only occur when an input hook
    /// fires twice for one physical action. Anything less strict corrupts
    /// recordings: clicks are stored as separate press/release events, so
    /// position-based "debouncing" would strip intentional double-clicks and
    /// unbalance press/release pairs.
    fn remove_duplicate_events(&self, events: Vec<InputEvent>) -> Vec<InputEvent> {
        let mut result: Vec<InputEvent> = Vec::with_capacity(events.len());
        let mut prev_json: Option<String> = None;

        for event in events {
            let json = serde_json::to_string(&event).unwrap_or_default();
            if prev_json.as_deref() == Some(json.as_str()) && event.timestamp().is_some() {
                continue; // identical payload + identical timestamp = hook glitch
            }
            prev_json = Some(json);
            result.push(event);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::InputEvent;

    fn click(x: i32, y: i32, button: u8, ts: Option<u64>) -> InputEvent {
        InputEvent::MouseClick {
            x,
            y,
            button,
            element: None,
            timestamp: ts,
            retry_count: None,
            semantic_tag: None,
            self_heal: None,
        }
    }

    #[test]
    fn double_click_survives_optimization() {
        // A real double-click: two press/release pairs at the same point,
        // 150ms apart. Optimization must not eat any of it.
        let events = vec![
            click(100, 100, 0, Some(1_000)),
            click(100, 100, 1, Some(1_080)),
            click(100, 100, 0, Some(1_150)),
            click(100, 100, 1, Some(1_230)),
        ];

        let optimized = WorkflowOptimizer::new().optimize(&events).unwrap();
        assert_eq!(optimized.len(), 4, "double-click events must be preserved");
    }

    #[test]
    fn exact_hook_duplicates_are_removed() {
        let events = vec![
            click(100, 100, 0, Some(1_000)),
            click(100, 100, 0, Some(1_000)), // identical payload + timestamp
            click(100, 100, 1, Some(1_080)),
        ];

        let optimized = WorkflowOptimizer::new().optimize(&events).unwrap();
        assert_eq!(optimized.len(), 2);
    }

    #[test]
    fn untimestamped_repeats_are_kept() {
        // Without timestamps we can't distinguish a glitch from intent —
        // keep everything rather than corrupt the workflow.
        let events = vec![click(5, 5, 0, None), click(5, 5, 0, None)];
        let optimized = WorkflowOptimizer::new().optimize(&events).unwrap();
        assert_eq!(optimized.len(), 2);
    }

    #[test]
    fn consecutive_delays_merge() {
        let events = vec![
            InputEvent::Delay {
                ms: 200,
                timestamp: None,
            },
            InputEvent::Delay {
                ms: 300,
                timestamp: None,
            },
            click(10, 10, 0, Some(1_000)),
        ];

        let optimized = WorkflowOptimizer::new().optimize(&events).unwrap();
        assert_eq!(optimized.len(), 2);
        match &optimized[0] {
            InputEvent::Delay { ms, .. } => assert_eq!(*ms, 500),
            other => panic!("expected merged delay, got {:?}", other),
        }
    }
}
