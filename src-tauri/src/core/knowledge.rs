//! Knowledge Base - Smart AI Parrot Helper/Geek Mode
//! Stores learned patterns, insights, and provides proactive suggestions.

use crate::core::events::InputEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A learned pattern from observing user behavior
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LearnedPattern {
    pub id: String,
    pub app_name: String,
    pub pattern_type: LearningPatternType,
    pub description: String,
    pub trigger_conditions: Vec<String>,
    pub suggested_actions: Vec<String>,
    pub confidence: f32,
    pub first_seen: u64,
    pub last_seen: u64,
    pub occurrence_count: u32,
    pub events: Vec<InputEvent>,
    pub geek_details: Option<GeekDetails>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum LearningPatternType {
    RepetitiveSequence,
    FrequentAppUsage,
    TimeBasedPattern,
    ContextSwitchPattern,
    ErrorRecoveryPattern,
    ShortcutDiscovery,
}

/// Technical details for "Geek Mode" - power user insights
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GeekDetails {
    pub event_timing_analysis: Vec<EventTiming>,
    pub system_calls_traced: Vec<String>,
    pub alternative_shortcuts: Vec<String>,
    pub performance_metrics: PerformanceMetrics,
    pub raw_ax_tree_snapshots: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventTiming {
    pub event_index: usize,
    pub timestamp_ms: u64,
    pub delay_before_ms: u64,
    pub estimated_action: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PerformanceMetrics {
    pub total_duration_ms: u64,
    pub avg_delay_ms: f64,
    pub bottleneck_events: Vec<usize>,
}

/// Knowledge base for storing learned automation insights
pub struct KnowledgeBase {
    patterns: Arc<Mutex<Vec<LearnedPattern>>>,
    app_usage: Arc<Mutex<HashMap<String, AppUsageStats>>>,
    observer_active: Arc<Mutex<bool>>,
    observer_interval_ms: Arc<Mutex<u64>>,
}

impl KnowledgeBase {
    pub fn new() -> Self {
        KnowledgeBase {
            patterns: Arc::new(Mutex::new(Vec::new())),
            app_usage: Arc::new(Mutex::new(HashMap::new())),
            observer_active: Arc::new(Mutex::new(false)),
            observer_interval_ms: Arc::new(Mutex::new(1000)),
        }
    }

    /// Start the observer mode - watch user behavior
    pub fn start_observer(&self) {
        *self.observer_active.lock().unwrap() = true;
    }

    /// Stop the observer mode
    pub fn stop_observer(&self) {
        *self.observer_active.lock().unwrap() = false;
    }

    /// Check if observer is active
    pub fn is_observer_active(&self) -> bool {
        *self.observer_active.lock().unwrap()
    }

    /// Set observer interval
    pub fn set_observer_interval(&self, interval_ms: u64) {
        *self.observer_interval_ms.lock().unwrap() = interval_ms;
    }

    /// Record an observed pattern
    pub fn observe_pattern(&self, pattern: LearnedPattern) {
        let mut patterns = self.patterns.lock().unwrap();
        patterns.push(pattern);
    }

    /// Analyze recorded events and extract patterns
    pub fn analyze_observed_events(
        &self,
        events: &[InputEvent],
        app_name: &str,
    ) -> Vec<LearnedPattern> {
        let mut patterns = Vec::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Detect repetitive sequences
        let sequence = self.detect_sequence_pattern(events, app_name, now);
        if let Some(p) = sequence {
            patterns.push(p);
        }

        // Detect potential shortcut discoveries
        let shortcuts = self.detect_shortcut_patterns(events, app_name, now);
        patterns.extend(shortcuts);

        patterns
    }

    /// Detect a sequence pattern
    fn detect_sequence_pattern(
        &self,
        events: &[InputEvent],
        app_name: &str,
        timestamp: u64,
    ) -> Option<LearnedPattern> {
        if events.len() < 3 {
            return None;
        }

        // Count event types
        let click_count = events
            .iter()
            .filter(|e| matches!(e, InputEvent::MouseClick { .. }))
            .count();
        let key_count = events
            .iter()
            .filter(|e| matches!(e, InputEvent::Key { .. }))
            .count();
        let sequence_length = events.len();

        // Only suggest if there's a meaningful pattern
        if click_count + key_count > 2 {
            let confidence = (events.len() as f32 / 10.0).min(1.0);

            Some(LearnedPattern {
                id: format!("seq_{}_{}", app_name, timestamp),
                app_name: app_name.to_string(),
                pattern_type: LearningPatternType::RepetitiveSequence,
                description: format!(
                    "Detected {}-event sequence with {} clicks and {} keystrokes",
                    sequence_length, click_count, key_count
                ),
                trigger_conditions: vec!["app_focus".to_string()],
                suggested_actions: vec![
                    "Save as workflow".to_string(),
                    "Add to automation suggestions".to_string(),
                ],
                confidence,
                first_seen: timestamp,
                last_seen: timestamp,
                occurrence_count: 1,
                events: events.to_vec(),
                geek_details: None,
            })
        } else {
            None
        }
    }

    /// Detect potential keyboard shortcuts
    fn detect_shortcut_patterns(
        &self,
        events: &[InputEvent],
        app_name: &str,
        timestamp: u64,
    ) -> Vec<LearnedPattern> {
        let mut patterns = Vec::new();

        // Look for key combinations (multiple keys without delay)
        let mut i = 0;
        while i < events.len() {
            if let InputEvent::Key { modifiers, .. } = &events[i]
                && *modifiers > 0
                && i + 1 < events.len()
                && let InputEvent::Key { .. } = &events[i + 1]
            {
                // Likely a shortcut
                patterns.push(LearnedPattern {
                    id: format!("shortcut_{}_{}", app_name, i),
                    app_name: app_name.to_string(),
                    pattern_type: LearningPatternType::ShortcutDiscovery,
                    description: format!(
                        "Potential keyboard shortcut detected: {} modifier + key",
                        modifiers
                    ),
                    trigger_conditions: vec!["app_focus".to_string()],
                    suggested_actions: vec![
                        "Save as keyboard macro".to_string(),
                        "Show in Geek Mode".to_string(),
                    ],
                    confidence: 0.75,
                    first_seen: timestamp,
                    last_seen: timestamp,
                    occurrence_count: 1,
                    events: vec![],
                    geek_details: None,
                });
                i += 2;
                continue;
            }
            i += 1;
        }

        patterns
    }

    /// Get all learned patterns
    pub fn get_patterns(&self) -> Vec<LearnedPattern> {
        self.patterns.lock().unwrap().clone()
    }

    /// Get patterns for a specific app
    pub fn get_app_patterns(&self, app_name: &str) -> Vec<LearnedPattern> {
        self.patterns
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.app_name == app_name)
            .cloned()
            .collect()
    }

    /// Get proactive suggestions based on learned patterns
    pub fn get_suggestions(&self) -> Vec<ProactiveSuggestion> {
        let patterns = self.patterns.lock().unwrap();

        patterns
            .iter()
            .filter(|p| p.confidence > 0.7 && p.occurrence_count > 1)
            .map(|p| ProactiveSuggestion {
                pattern_id: p.id.clone(),
                suggestion: format!(
                    "🤖 I've noticed you do '{}' frequently. Want me to automate this?",
                    p.description
                ),
                suggested_workflow_name: format!("Auto-{}", p.app_name),
                confidence: p.confidence,
            })
            .collect()
    }

    /// Track app usage
    pub fn track_app_usage(&self, app_name: &str) {
        let mut usage = self.app_usage.lock().unwrap();
        let stats = usage.entry(app_name.to_string()).or_insert(AppUsageStats {
            app_name: app_name.to_string(),
            usage_count: 0,
            last_used: 0,
        });
        stats.usage_count += 1;
        stats.last_used = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// Get app usage statistics
    pub fn get_app_usage(&self) -> Vec<AppUsageStats> {
        self.app_usage.lock().unwrap().values().cloned().collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppUsageStats {
    pub app_name: String,
    pub usage_count: u32,
    pub last_used: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProactiveSuggestion {
    pub pattern_id: String,
    pub suggestion: String,
    pub suggested_workflow_name: String,
    pub confidence: f32,
}

impl Default for KnowledgeBase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::KeyAction;

    fn click_event() -> InputEvent {
        InputEvent::MouseClick {
            x: 0,
            y: 0,
            button: 0,
            element: None,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
            self_heal: None,
        }
    }

    fn key_event(modifiers: u8, chars: &str) -> InputEvent {
        InputEvent::Key {
            code: 0,
            chars: chars.to_string(),
            modifiers,
            action: KeyAction::Down,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
        }
    }

    #[test]
    fn observer_state_defaults_and_toggle() {
        let kb = KnowledgeBase::new();
        assert!(!kb.is_observer_active());

        kb.start_observer();
        assert!(kb.is_observer_active());

        kb.stop_observer();
        assert!(!kb.is_observer_active());

        kb.set_observer_interval(5000);
        assert_eq!(*kb.observer_interval_ms.lock().unwrap(), 5000);
    }

    #[test]
    fn analyze_observed_events_too_short_detects_nothing() {
        let kb = KnowledgeBase::new();
        let events = vec![click_event(), click_event()];
        let patterns = kb.analyze_observed_events(&events, "TestApp");
        assert!(patterns.is_empty());
    }

    #[test]
    fn analyze_observed_events_detects_repetitive_sequence() {
        let kb = KnowledgeBase::new();
        let events = vec![click_event(), click_event(), click_event(), click_event()];
        let patterns = kb.analyze_observed_events(&events, "TestApp");

        let sequence = patterns
            .iter()
            .find(|p| matches!(p.pattern_type, LearningPatternType::RepetitiveSequence))
            .expect("expected a repetitive sequence pattern");

        assert_eq!(sequence.app_name, "TestApp");
        assert_eq!(sequence.occurrence_count, 1);
        assert_eq!(sequence.events.len(), events.len());
        assert!(sequence.confidence > 0.0 && sequence.confidence <= 1.0);
        assert!(sequence.description.contains("4-event sequence"));
    }

    #[test]
    fn analyze_observed_events_detects_shortcut() {
        let kb = KnowledgeBase::new();
        // A modifier-key event followed immediately by another key event
        let events = vec![
            key_event(1, "c"),
            key_event(0, "v"),
            click_event(),
            click_event(),
        ];
        let patterns = kb.analyze_observed_events(&events, "Editor");

        let shortcut = patterns
            .iter()
            .find(|p| matches!(p.pattern_type, LearningPatternType::ShortcutDiscovery))
            .expect("expected a shortcut pattern");

        assert_eq!(shortcut.app_name, "Editor");
        assert_eq!(shortcut.confidence, 0.75);
    }

    #[test]
    fn observe_and_retrieve_patterns() {
        let kb = KnowledgeBase::new();
        let pattern = LearnedPattern {
            id: "p1".to_string(),
            app_name: "AppA".to_string(),
            pattern_type: LearningPatternType::RepetitiveSequence,
            description: "desc".to_string(),
            trigger_conditions: vec![],
            suggested_actions: vec![],
            confidence: 0.9,
            first_seen: 0,
            last_seen: 0,
            occurrence_count: 2,
            events: vec![],
            geek_details: None,
        };
        kb.observe_pattern(pattern.clone());

        let all = kb.get_patterns();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "p1");

        let app_a = kb.get_app_patterns("AppA");
        assert_eq!(app_a.len(), 1);

        let app_b = kb.get_app_patterns("AppB");
        assert!(app_b.is_empty());
    }

    #[test]
    fn get_suggestions_filters_by_confidence_and_occurrence() {
        let kb = KnowledgeBase::new();

        // High confidence, repeated -> should produce a suggestion
        kb.observe_pattern(LearnedPattern {
            id: "strong".to_string(),
            app_name: "AppA".to_string(),
            pattern_type: LearningPatternType::RepetitiveSequence,
            description: "do the thing".to_string(),
            trigger_conditions: vec![],
            suggested_actions: vec![],
            confidence: 0.9,
            first_seen: 0,
            last_seen: 0,
            occurrence_count: 3,
            events: vec![],
            geek_details: None,
        });

        // Low confidence -> filtered out
        kb.observe_pattern(LearnedPattern {
            id: "weak_confidence".to_string(),
            app_name: "AppA".to_string(),
            pattern_type: LearningPatternType::RepetitiveSequence,
            description: "maybe".to_string(),
            trigger_conditions: vec![],
            suggested_actions: vec![],
            confidence: 0.5,
            first_seen: 0,
            last_seen: 0,
            occurrence_count: 5,
            events: vec![],
            geek_details: None,
        });

        // Only seen once -> filtered out
        kb.observe_pattern(LearnedPattern {
            id: "single_occurrence".to_string(),
            app_name: "AppA".to_string(),
            pattern_type: LearningPatternType::RepetitiveSequence,
            description: "once".to_string(),
            trigger_conditions: vec![],
            suggested_actions: vec![],
            confidence: 0.95,
            first_seen: 0,
            last_seen: 0,
            occurrence_count: 1,
            events: vec![],
            geek_details: None,
        });

        let suggestions = kb.get_suggestions();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].pattern_id, "strong");
        assert_eq!(suggestions[0].suggested_workflow_name, "Auto-AppA");
        assert!(suggestions[0].suggestion.contains("do the thing"));
    }

    #[test]
    fn track_app_usage_accumulates_counts() {
        let kb = KnowledgeBase::new();
        kb.track_app_usage("AppA");
        kb.track_app_usage("AppA");
        kb.track_app_usage("AppB");

        let usage = kb.get_app_usage();
        let app_a = usage.iter().find(|u| u.app_name == "AppA").unwrap();
        let app_b = usage.iter().find(|u| u.app_name == "AppB").unwrap();

        assert_eq!(app_a.usage_count, 2);
        assert_eq!(app_b.usage_count, 1);
        assert!(app_a.last_used > 0);
    }
}
