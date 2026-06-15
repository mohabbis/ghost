//! Execution logging and history tracking.
//! Stores replay results for analytics and debugging.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

/// Execution record for a single workflow run
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecutionRecord {
    pub id: String,
    pub workflow_name: String,
    pub status: ExecutionStatus,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub duration_ms: Option<u64>,
    pub events_processed: usize,
    pub error_message: Option<String>,
    pub failure_screenshot: Option<String>, // Path to screenshot on failure
    pub metadata: ExecutionMetadata,
}

/// Execution status
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ExecutionStatus {
    Running,
    Success,
    Failed,
    Cancelled,
}

/// Additional metadata for execution
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ExecutionMetadata {
    pub avg_event_latency_ms: f32,
    pub failure_hotspot: Option<String>,
    pub replay_speed: f32,
    pub device_info: String,
    pub os_version: String,
}

impl ExecutionRecord {
    /// Create a new execution record
    pub fn new(workflow_name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            workflow_name,
            status: ExecutionStatus::Running,
            start_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            end_time: None,
            duration_ms: None,
            events_processed: 0,
            error_message: None,
            failure_screenshot: None,
            metadata: ExecutionMetadata::default(),
        }
    }

    /// Mark execution as successful
    pub fn complete(&mut self, events_processed: usize, duration_ms: u64) {
        self.status = ExecutionStatus::Success;
        self.end_time = Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
        self.duration_ms = Some(duration_ms);
        self.events_processed = events_processed;
    }

    /// Mark execution as failed
    pub fn fail(&mut self, error: &str, screenshot_path: Option<String>) {
        self.status = ExecutionStatus::Failed;
        self.end_time = Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
        self.error_message = Some(error.to_string());
        self.failure_screenshot = screenshot_path;
    }

    /// Mark execution as cancelled
    pub fn cancel(&mut self) {
        self.status = ExecutionStatus::Cancelled;
        self.end_time = Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }
}

/// Execution history manager
pub struct ExecutionHistory {
    logs_dir: PathBuf,
}

impl ExecutionHistory {
    /// Create a new execution history manager
    pub fn new() -> anyhow::Result<Self> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        let logs_dir = data_dir.join("ghost").join("logs");
        fs::create_dir_all(&logs_dir)?;

        Ok(Self { logs_dir })
    }

    /// Save an execution record
    pub fn save(&self, record: &ExecutionRecord) -> anyhow::Result<()> {
        let file_path = self.logs_dir.join(format!("{}.json", record.id));
        let json = serde_json::to_string_pretty(record)?;
        fs::write(&file_path, json)?;
        Ok(())
    }

    /// Load all execution history for a workflow
    pub fn get_history(&self, workflow_name: &str) -> anyhow::Result<Vec<ExecutionRecord>> {
        let mut records = Vec::new();

        if !self.logs_dir.exists() {
            return Ok(records);
        }

        for entry in fs::read_dir(&self.logs_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(record) = serde_json::from_str::<ExecutionRecord>(&content) {
                        if record.workflow_name == workflow_name {
                            records.push(record);
                        }
                    }
                }
            }
        }

        // Sort by start time, newest first
        records.sort_by_key(|r| std::cmp::Reverse(r.start_time));

        Ok(records)
    }

    /// Load all execution records
    pub fn get_all_records(&self, limit: Option<usize>) -> anyhow::Result<Vec<ExecutionRecord>> {
        let mut records = Vec::new();

        if !self.logs_dir.exists() {
            return Ok(records);
        }

        for entry in fs::read_dir(&self.logs_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(record) = serde_json::from_str::<ExecutionRecord>(&content) {
                        records.push(record);
                    }
                }
            }
        }

        // Sort by start time, newest first
        records.sort_by_key(|r| std::cmp::Reverse(r.start_time));

        if let Some(limit) = limit {
            records.truncate(limit);
        }

        Ok(records)
    }

    /// Calculate success rate for a workflow
    pub fn get_success_rate(&self, workflow_name: &str) -> anyhow::Result<f32> {
        let records = self.get_history(workflow_name)?;
        let total = records.len();

        if total == 0 {
            return Ok(1.0);
        }

        let success_count = records
            .iter()
            .filter(|r| r.status == ExecutionStatus::Success)
            .count();

        Ok(success_count as f32 / total as f32)
    }

    /// Calculate average duration for a workflow
    pub fn get_avg_duration(&self, workflow_name: &str) -> anyhow::Result<u64> {
        let records = self.get_history(workflow_name)?;

        let durations: Vec<u64> = records.iter().filter_map(|r| r.duration_ms).collect();

        if durations.is_empty() {
            return Ok(0);
        }

        let sum: u64 = durations.iter().sum();
        Ok(sum / durations.len() as u64)
    }

    /// Find failure hotspots
    pub fn get_failure_hotspots(&self, workflow_name: &str) -> anyhow::Result<Vec<String>> {
        let records = self.get_history(workflow_name)?;

        let mut hotspots = Vec::new();
        for record in records {
            if let Some(error) = &record.error_message {
                // Extract element name from error if possible
                if error.contains("element") {
                    hotspots.push(format!("Element error: {}", error));
                } else {
                    hotspots.push(error.clone());
                }
            }
        }

        Ok(hotspots)
    }

    /// Clear old logs (retention policy)
    pub fn cleanup_old_logs(&self, older_than_days: u64) -> anyhow::Result<()> {
        let cutoff = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs()
            - (older_than_days * 24 * 60 * 60);

        for entry in fs::read_dir(&self.logs_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(record) = serde_json::from_str::<ExecutionRecord>(&content) {
                    if record.start_time < cutoff {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for ExecutionHistory {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback: use temp directory
            // We need to create a valid ExecutionHistory instance
            // Use the system temp dir as fallback
            let logs_dir = std::env::temp_dir().join("ghost").join("logs");
            let _ = std::fs::create_dir_all(&logs_dir);
            Self { logs_dir }
        })
    }
}

/// Thread-safe execution tracker
pub mod tracker {
    use super::{ExecutionHistory, ExecutionRecord};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    /// Active execution tracking
    #[derive(Clone)]
    pub struct ExecutionTracker {
        inner: Arc<Mutex<TrackerInner>>,
    }

    struct TrackerInner {
        active: Option<ExecutionRecord>,
        start_time: Instant,
        events_count: usize,
        history: ExecutionHistory,
    }

    impl ExecutionTracker {
        pub fn new(history: ExecutionHistory) -> Self {
            Self {
                inner: Arc::new(Mutex::new(TrackerInner {
                    active: None,
                    start_time: Instant::now(),
                    events_count: 0,
                    history,
                })),
            }
        }

        pub fn start(&self, workflow_name: String) {
            let mut inner = self.inner.lock().unwrap();
            inner.active = Some(ExecutionRecord::new(workflow_name));
            inner.start_time = Instant::now();
            inner.events_count = 0;
        }

        pub fn increment(&self) {
            let mut inner = self.inner.lock().unwrap();
            inner.events_count += 1;
        }

        pub fn complete(&self) -> anyhow::Result<()> {
            let mut inner = self.inner.lock().unwrap();
            let events_count = inner.events_count;
            let duration_ms = inner.start_time.elapsed().as_millis() as u64;
            if let Some(ref mut record) = inner.active {
                record.complete(events_count, duration_ms);
            }
            if let Some(record) = inner.active.take() {
                inner.history.save(&record)?;
            }
            Ok(())
        }

        pub fn fail(&self, error: &str, screenshot_path: Option<String>) -> anyhow::Result<()> {
            let mut inner = self.inner.lock().unwrap();
            if let Some(ref mut record) = inner.active {
                record.fail(error, screenshot_path.clone());
            }
            if let Some(record) = inner.active.take() {
                inner.history.save(&record)?;
            }
            Ok(())
        }

        pub fn cancel(&self) -> anyhow::Result<()> {
            let mut inner = self.inner.lock().unwrap();
            if let Some(ref mut record) = inner.active {
                record.cancel();
            }
            if let Some(record) = inner.active.take() {
                inner.history.save(&record)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an `ExecutionHistory` rooted in a fresh temp directory, isolated
    /// from the real `dirs::data_dir()` used by `ExecutionHistory::new()`.
    fn temp_history(name: &str) -> ExecutionHistory {
        let logs_dir = std::env::temp_dir().join(format!(
            "ghost_execution_test_{}_{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&logs_dir);
        fs::create_dir_all(&logs_dir).unwrap();
        ExecutionHistory { logs_dir }
    }

    fn cleanup(history: &ExecutionHistory) {
        let _ = fs::remove_dir_all(&history.logs_dir);
    }

    fn record_with(
        workflow_name: &str,
        status: ExecutionStatus,
        start_time: u64,
        duration_ms: Option<u64>,
        error_message: Option<&str>,
    ) -> ExecutionRecord {
        ExecutionRecord {
            id: uuid::Uuid::new_v4().to_string(),
            workflow_name: workflow_name.to_string(),
            status,
            start_time,
            end_time: None,
            duration_ms,
            events_processed: 0,
            error_message: error_message.map(|s| s.to_string()),
            failure_screenshot: None,
            metadata: ExecutionMetadata::default(),
        }
    }

    #[test]
    fn execution_record_lifecycle() {
        let mut record = ExecutionRecord::new("wf".to_string());
        assert_eq!(record.status, ExecutionStatus::Running);
        assert!(record.end_time.is_none());

        record.complete(5, 1234);
        assert_eq!(record.status, ExecutionStatus::Success);
        assert_eq!(record.events_processed, 5);
        assert_eq!(record.duration_ms, Some(1234));
        assert!(record.end_time.is_some());
    }

    #[test]
    fn execution_record_fail_and_cancel() {
        let mut record = ExecutionRecord::new("wf".to_string());
        record.fail("boom", Some("shot.png".to_string()));
        assert_eq!(record.status, ExecutionStatus::Failed);
        assert_eq!(record.error_message, Some("boom".to_string()));
        assert_eq!(record.failure_screenshot, Some("shot.png".to_string()));

        let mut record2 = ExecutionRecord::new("wf".to_string());
        record2.cancel();
        assert_eq!(record2.status, ExecutionStatus::Cancelled);
        assert!(record2.end_time.is_some());
    }

    #[test]
    fn save_and_get_history_filters_by_workflow() {
        let history = temp_history("save_get");

        let r1 = record_with("wf-a", ExecutionStatus::Success, 100, Some(50), None);
        let r2 = record_with("wf-a", ExecutionStatus::Failed, 200, Some(150), Some("err"));
        let r3 = record_with("wf-b", ExecutionStatus::Success, 300, Some(100), None);

        history.save(&r1).unwrap();
        history.save(&r2).unwrap();
        history.save(&r3).unwrap();

        let wf_a = history.get_history("wf-a").unwrap();
        assert_eq!(wf_a.len(), 2);
        // Sorted newest-first by start_time
        assert_eq!(wf_a[0].start_time, 200);
        assert_eq!(wf_a[1].start_time, 100);

        let wf_b = history.get_history("wf-b").unwrap();
        assert_eq!(wf_b.len(), 1);

        let wf_missing = history.get_history("wf-does-not-exist").unwrap();
        assert!(wf_missing.is_empty());

        cleanup(&history);
    }

    #[test]
    fn get_all_records_sorted_and_limited() {
        let history = temp_history("all_records");

        let r1 = record_with("wf-a", ExecutionStatus::Success, 100, Some(50), None);
        let r2 = record_with("wf-b", ExecutionStatus::Success, 300, Some(100), None);
        let r3 = record_with("wf-c", ExecutionStatus::Success, 200, Some(75), None);

        history.save(&r1).unwrap();
        history.save(&r2).unwrap();
        history.save(&r3).unwrap();

        let all = history.get_all_records(None).unwrap();
        assert_eq!(all.len(), 3);
        // Newest first
        assert_eq!(all[0].start_time, 300);
        assert_eq!(all[1].start_time, 200);
        assert_eq!(all[2].start_time, 100);

        let limited = history.get_all_records(Some(2)).unwrap();
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].start_time, 300);
        assert_eq!(limited[1].start_time, 200);

        cleanup(&history);
    }

    #[test]
    fn success_rate_with_no_records_is_one() {
        let history = temp_history("success_rate_empty");
        let rate = history.get_success_rate("nonexistent").unwrap();
        assert_eq!(rate, 1.0);
        cleanup(&history);
    }

    #[test]
    fn success_rate_computed_from_mixed_records() {
        let history = temp_history("success_rate_mixed");

        history
            .save(&record_with(
                "wf",
                ExecutionStatus::Success,
                1,
                Some(10),
                None,
            ))
            .unwrap();
        history
            .save(&record_with(
                "wf",
                ExecutionStatus::Success,
                2,
                Some(20),
                None,
            ))
            .unwrap();
        history
            .save(&record_with(
                "wf",
                ExecutionStatus::Failed,
                3,
                Some(30),
                Some("err"),
            ))
            .unwrap();
        history
            .save(&record_with(
                "wf",
                ExecutionStatus::Cancelled,
                4,
                None,
                None,
            ))
            .unwrap();

        let rate = history.get_success_rate("wf").unwrap();
        assert!((rate - 0.5).abs() < f32::EPSILON);

        cleanup(&history);
    }

    #[test]
    fn avg_duration_ignores_missing_durations() {
        let history = temp_history("avg_duration");

        history
            .save(&record_with(
                "wf",
                ExecutionStatus::Success,
                1,
                Some(100),
                None,
            ))
            .unwrap();
        history
            .save(&record_with(
                "wf",
                ExecutionStatus::Success,
                2,
                Some(200),
                None,
            ))
            .unwrap();
        // No duration_ms - should be excluded from the average
        history
            .save(&record_with(
                "wf",
                ExecutionStatus::Cancelled,
                3,
                None,
                None,
            ))
            .unwrap();

        let avg = history.get_avg_duration("wf").unwrap();
        assert_eq!(avg, 150);

        cleanup(&history);
    }

    #[test]
    fn avg_duration_with_no_records_is_zero() {
        let history = temp_history("avg_duration_empty");
        let avg = history.get_avg_duration("nonexistent").unwrap();
        assert_eq!(avg, 0);
        cleanup(&history);
    }

    #[test]
    fn failure_hotspots_collected_in_order() {
        let history = temp_history("failure_hotspots");

        history
            .save(&record_with(
                "wf",
                ExecutionStatus::Success,
                1,
                Some(10),
                None,
            ))
            .unwrap();
        history
            .save(&record_with(
                "wf",
                ExecutionStatus::Failed,
                2,
                Some(20),
                Some("element not found"),
            ))
            .unwrap();
        history
            .save(&record_with(
                "wf",
                ExecutionStatus::Failed,
                3,
                Some(30),
                Some("network timeout"),
            ))
            .unwrap();

        let hotspots = history.get_failure_hotspots("wf").unwrap();
        assert_eq!(hotspots.len(), 2);
        // Sorted newest-first, so "network timeout" (start_time 3) comes first
        assert_eq!(hotspots[0], "network timeout");
        assert_eq!(hotspots[1], "Element error: element not found");

        cleanup(&history);
    }

    #[test]
    fn cleanup_old_logs_removes_only_old_records() {
        let history = temp_history("cleanup_logs");

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let old_time = now - 30 * 24 * 60 * 60; // 30 days ago

        let old_record = record_with("wf", ExecutionStatus::Success, old_time, Some(10), None);
        let new_record = record_with("wf", ExecutionStatus::Success, now, Some(20), None);

        history.save(&old_record).unwrap();
        history.save(&new_record).unwrap();

        // Retain records from the last 7 days
        history.cleanup_old_logs(7).unwrap();

        let remaining = history.get_history("wf").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].start_time, now);

        cleanup(&history);
    }

    #[test]
    fn execution_tracker_lifecycle_persists_record() {
        let history = temp_history("tracker");
        let tracker = tracker::ExecutionTracker::new(ExecutionHistory {
            logs_dir: history.logs_dir.clone(),
        });

        tracker.start("tracked-wf".to_string());
        tracker.increment();
        tracker.increment();
        tracker.complete().unwrap();

        let records = history.get_history("tracked-wf").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, ExecutionStatus::Success);
        assert_eq!(records[0].events_processed, 2);

        cleanup(&history);
    }
}
