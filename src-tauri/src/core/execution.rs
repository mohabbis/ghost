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
                .as_secs()
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
                .as_secs()
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
                .as_secs()
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
        records.sort_by(|a, b| b.start_time.cmp(&a.start_time));

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
        records.sort_by(|a, b| b.start_time.cmp(&a.start_time));

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

        let success_count = records.iter()
            .filter(|r| r.status == ExecutionStatus::Success)
            .count();

        Ok(success_count as f32 / total as f32)
    }

    /// Calculate average duration for a workflow
    pub fn get_avg_duration(&self, workflow_name: &str) -> anyhow::Result<u64> {
        let records = self.get_history(workflow_name)?;
        
        let durations: Vec<u64> = records.iter()
            .filter_map(|r| r.duration_ms)
            .collect();

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
    use super::{ExecutionHistory, ExecutionRecord, ExecutionStatus};
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
            if let Some(ref mut record) = inner.active {
                record.complete(inner.events_count, inner.start_time.elapsed().as_millis() as u64);
                inner.history.save(record)?;
            }
            inner.active = None;
            Ok(())
        }

        pub fn fail(&self, error: &str, screenshot_path: Option<String>) -> anyhow::Result<()> {
            let mut inner = self.inner.lock().unwrap();
            if let Some(ref mut record) = inner.active {
                record.fail(error, screenshot_path);
                inner.history.save(record)?;
            }
            inner.active = None;
            Ok(())
        }

        pub fn cancel(&self) -> anyhow::Result<()> {
            let mut inner = self.inner.lock().unwrap();
            if let Some(ref mut record) = inner.active {
                record.cancel();
                inner.history.save(record)?;
            }
            inner.active = None;
            Ok(())
        }
    }
}