//! Opt-in telemetry and analytics for Ghost
//! Helps improve the app while respecting user privacy

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Event type (e.g., "workflow_recorded", "workflow_replayed")
    pub event_type: String,
    /// Timestamp in seconds since epoch
    pub timestamp: u64,
    /// Anonymized properties
    pub properties: std::collections::HashMap<String, serde_json::Value>,
    /// Session ID (generated per app launch)
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    /// Total workflows recorded
    pub workflows_recorded: u64,
    /// Total workflows replayed
    pub workflows_replayed: u64,
    /// Total recording time (seconds)
    pub total_recording_time_secs: u64,
    /// Total replay time (seconds)
    pub total_replay_time_secs: u64,
    /// Average workflow length (events)
    pub avg_workflow_length: f64,
    /// Most used features
    pub feature_usage: std::collections::HashMap<String, u64>,
    /// Error counts by type
    pub error_counts: std::collections::HashMap<String, u64>,
}

pub struct TelemetryManager {
    enabled: Arc<Mutex<bool>>,
    session_id: String,
    events: Arc<Mutex<Vec<TelemetryEvent>>>,
    stats: Arc<Mutex<UsageStats>>,
}

impl TelemetryManager {
    pub fn new(enabled: bool) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();

        Self {
            enabled: Arc::new(Mutex::new(enabled)),
            session_id,
            events: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(UsageStats::default())),
        }
    }

    /// Check if telemetry is enabled
    pub fn is_enabled(&self) -> bool {
        *self.enabled.lock().unwrap()
    }

    /// Enable or disable telemetry
    pub fn set_enabled(&self, enabled: bool) {
        *self.enabled.lock().unwrap() = enabled;

        if !enabled {
            // Clear all collected data when disabled
            self.events.lock().unwrap().clear();
        }
    }

    /// Track an event (only if enabled)
    pub fn track_event(
        &self,
        event_type: impl Into<String>,
        properties: std::collections::HashMap<String, serde_json::Value>,
    ) {
        if !self.is_enabled() {
            return;
        }

        let event = TelemetryEvent {
            event_type: event_type.into(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            properties,
            session_id: self.session_id.clone(),
        };

        self.events.lock().unwrap().push(event);
    }

    /// Track workflow recording
    pub fn track_workflow_recorded(&self, event_count: usize, duration_secs: u64) {
        if !self.is_enabled() {
            return;
        }

        let mut props = std::collections::HashMap::new();
        props.insert("event_count".to_string(), serde_json::json!(event_count));
        props.insert(
            "duration_secs".to_string(),
            serde_json::json!(duration_secs),
        );

        self.track_event("workflow_recorded", props);

        // Update stats
        let mut stats = self.stats.lock().unwrap();
        stats.workflows_recorded += 1;
        stats.total_recording_time_secs += duration_secs;
        stats.update_avg_workflow_length(event_count);
    }

    /// Track workflow replay
    pub fn track_workflow_replayed(&self, event_count: usize, duration_secs: u64, success: bool) {
        if !self.is_enabled() {
            return;
        }

        let mut props = std::collections::HashMap::new();
        props.insert("event_count".to_string(), serde_json::json!(event_count));
        props.insert(
            "duration_secs".to_string(),
            serde_json::json!(duration_secs),
        );
        props.insert("success".to_string(), serde_json::json!(success));

        self.track_event("workflow_replayed", props);

        // Update stats
        let mut stats = self.stats.lock().unwrap();
        stats.workflows_replayed += 1;
        stats.total_replay_time_secs += duration_secs;
    }

    /// Track feature usage
    pub fn track_feature_used(&self, feature: impl Into<String>) {
        if !self.is_enabled() {
            return;
        }

        let feature_name = feature.into();
        let mut props = std::collections::HashMap::new();
        props.insert(
            "feature".to_string(),
            serde_json::json!(feature_name.clone()),
        );

        self.track_event("feature_used", props);

        // Update stats
        let mut stats = self.stats.lock().unwrap();
        *stats.feature_usage.entry(feature_name).or_insert(0) += 1;
    }

    /// Track error occurrence
    pub fn track_error(&self, error_type: impl Into<String>, error_code: impl Into<String>) {
        if !self.is_enabled() {
            return;
        }

        let error_type_str = error_type.into();
        let mut props = std::collections::HashMap::new();
        props.insert(
            "error_type".to_string(),
            serde_json::json!(error_type_str.clone()),
        );
        props.insert(
            "error_code".to_string(),
            serde_json::json!(error_code.into()),
        );

        self.track_event("error_occurred", props);

        // Update stats
        let mut stats = self.stats.lock().unwrap();
        *stats.error_counts.entry(error_type_str).or_insert(0) += 1;
    }

    /// Get current usage statistics
    pub fn get_stats(&self) -> UsageStats {
        self.stats.lock().unwrap().clone()
    }

    /// Get all events (for export or analysis)
    pub fn get_events(&self) -> Vec<TelemetryEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Clear all telemetry data
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
        *self.stats.lock().unwrap() = UsageStats::default();
    }

    /// Export telemetry data as JSON
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        let data = serde_json::json!({
            "session_id": self.session_id,
            "stats": self.get_stats(),
            "events": self.get_events(),
        });

        serde_json::to_string_pretty(&data)
    }
}

impl Default for UsageStats {
    fn default() -> Self {
        Self {
            workflows_recorded: 0,
            workflows_replayed: 0,
            total_recording_time_secs: 0,
            total_replay_time_secs: 0,
            avg_workflow_length: 0.0,
            feature_usage: std::collections::HashMap::new(),
            error_counts: std::collections::HashMap::new(),
        }
    }
}

impl UsageStats {
    fn update_avg_workflow_length(&mut self, new_length: usize) {
        let total = self.avg_workflow_length * (self.workflows_recorded - 1) as f64;
        self.avg_workflow_length = (total + new_length as f64) / self.workflows_recorded as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_disabled_by_default() {
        let manager = TelemetryManager::new(false);
        assert!(!manager.is_enabled());

        manager.track_workflow_recorded(10, 60);
        assert_eq!(manager.get_events().len(), 0);
    }

    #[test]
    fn test_telemetry_enabled() {
        let manager = TelemetryManager::new(true);
        assert!(manager.is_enabled());

        manager.track_workflow_recorded(10, 60);
        assert_eq!(manager.get_events().len(), 1);

        let stats = manager.get_stats();
        assert_eq!(stats.workflows_recorded, 1);
    }

    #[test]
    fn test_clear_on_disable() {
        let manager = TelemetryManager::new(true);
        manager.track_workflow_recorded(10, 60);
        assert_eq!(manager.get_events().len(), 1);

        manager.set_enabled(false);
        assert_eq!(manager.get_events().len(), 0);
    }

    #[test]
    fn test_feature_tracking() {
        let manager = TelemetryManager::new(true);
        manager.track_feature_used("ai_optimize");
        manager.track_feature_used("ai_optimize");
        manager.track_feature_used("visual_check");

        let stats = manager.get_stats();
        assert_eq!(stats.feature_usage.get("ai_optimize"), Some(&2));
        assert_eq!(stats.feature_usage.get("visual_check"), Some(&1));
    }
}

// Made with Bob
