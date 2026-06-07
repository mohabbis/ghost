//! Performance monitoring and optimization utilities

use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// Performance metrics for operations
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Operation name
    pub operation: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Timestamp when operation started
    pub timestamp: u64,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Performance monitor for tracking operation timings
pub struct PerformanceMonitor {
    enabled: Arc<Mutex<bool>>,
    metrics: Arc<Mutex<Vec<PerformanceMetrics>>>,
    active_timers: Arc<Mutex<HashMap<String, Instant>>>,
}

impl PerformanceMonitor {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(Mutex::new(enabled)),
            metrics: Arc::new(Mutex::new(Vec::new())),
            active_timers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Check if monitoring is enabled
    pub fn is_enabled(&self) -> bool {
        *self.enabled.lock().unwrap()
    }
    
    /// Enable or disable monitoring
    pub fn set_enabled(&self, enabled: bool) {
        *self.enabled.lock().unwrap() = enabled;
    }
    
    /// Start timing an operation
    pub fn start_timer(&self, operation: impl Into<String>) {
        if !self.is_enabled() {
            return;
        }
        
        let op = operation.into();
        self.active_timers.lock().unwrap().insert(op, Instant::now());
    }
    
    /// Stop timing an operation and record metrics
    pub fn stop_timer(&self, operation: impl Into<String>) -> Option<u64> {
        if !self.is_enabled() {
            return None;
        }
        
        let op = operation.into();
        let start = self.active_timers.lock().unwrap().remove(&op)?;
        let duration = start.elapsed();
        let duration_ms = duration.as_millis() as u64;
        
        let metric = PerformanceMetrics {
            operation: op,
            duration_ms,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
        };
        
        self.metrics.lock().unwrap().push(metric);
        Some(duration_ms)
    }
    
    /// Stop timer with additional metadata
    pub fn stop_timer_with_metadata(
        &self,
        operation: impl Into<String>,
        metadata: HashMap<String, String>
    ) -> Option<u64> {
        if !self.is_enabled() {
            return None;
        }
        
        let op = operation.into();
        let start = self.active_timers.lock().unwrap().remove(&op)?;
        let duration = start.elapsed();
        let duration_ms = duration.as_millis() as u64;
        
        let metric = PerformanceMetrics {
            operation: op,
            duration_ms,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata,
        };
        
        self.metrics.lock().unwrap().push(metric);
        Some(duration_ms)
    }
    
    /// Get all recorded metrics
    pub fn get_metrics(&self) -> Vec<PerformanceMetrics> {
        self.metrics.lock().unwrap().clone()
    }
    
    /// Get metrics for a specific operation
    pub fn get_operation_metrics(&self, operation: &str) -> Vec<PerformanceMetrics> {
        self.metrics
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.operation == operation)
            .cloned()
            .collect()
    }
    
    /// Get average duration for an operation
    pub fn get_average_duration(&self, operation: &str) -> Option<u64> {
        let metrics = self.get_operation_metrics(operation);
        if metrics.is_empty() {
            return None;
        }
        
        let total: u64 = metrics.iter().map(|m| m.duration_ms).sum();
        Some(total / metrics.len() as u64)
    }
    
    /// Clear all metrics
    pub fn clear(&self) {
        self.metrics.lock().unwrap().clear();
        self.active_timers.lock().unwrap().clear();
    }
    
    /// Get performance summary
    pub fn get_summary(&self) -> PerformanceSummary {
        let metrics = self.get_metrics();
        let mut operation_stats: HashMap<String, OperationStats> = HashMap::new();
        
        for metric in metrics {
            let stats = operation_stats
                .entry(metric.operation.clone())
                .or_insert_with(|| OperationStats {
                    operation: metric.operation.clone(),
                    count: 0,
                    total_ms: 0,
                    min_ms: u64::MAX,
                    max_ms: 0,
                    avg_ms: 0.0,
                });
            
            stats.count += 1;
            stats.total_ms += metric.duration_ms;
            stats.min_ms = stats.min_ms.min(metric.duration_ms);
            stats.max_ms = stats.max_ms.max(metric.duration_ms);
            stats.avg_ms = stats.total_ms as f64 / stats.count as f64;
        }
        
        PerformanceSummary {
            operations: operation_stats.into_values().collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperationStats {
    pub operation: String,
    pub count: u64,
    pub total_ms: u64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub avg_ms: f64,
}

#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub operations: Vec<OperationStats>,
}

/// RAII timer that automatically stops when dropped
pub struct ScopedTimer<'a> {
    monitor: &'a PerformanceMonitor,
    operation: String,
}

impl<'a> ScopedTimer<'a> {
    pub fn new(monitor: &'a PerformanceMonitor, operation: impl Into<String>) -> Self {
        let op = operation.into();
        monitor.start_timer(op.clone());
        Self {
            monitor,
            operation: op,
        }
    }
}

impl<'a> Drop for ScopedTimer<'a> {
    fn drop(&mut self) {
        self.monitor.stop_timer(&self.operation);
    }
}

/// Macro for easy scoped timing
#[macro_export]
macro_rules! time_operation {
    ($monitor:expr, $operation:expr, $block:block) => {{
        let _timer = $crate::performance::ScopedTimer::new($monitor, $operation);
        $block
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_basic_timing() {
        let monitor = PerformanceMonitor::new(true);
        
        monitor.start_timer("test_op");
        thread::sleep(Duration::from_millis(10));
        let duration = monitor.stop_timer("test_op");
        
        assert!(duration.is_some());
        assert!(duration.unwrap() >= 10);
    }
    
    #[test]
    fn test_disabled_monitor() {
        let monitor = PerformanceMonitor::new(false);
        
        monitor.start_timer("test_op");
        let duration = monitor.stop_timer("test_op");
        
        assert!(duration.is_none());
        assert_eq!(monitor.get_metrics().len(), 0);
    }
    
    #[test]
    fn test_average_duration() {
        let monitor = PerformanceMonitor::new(true);
        
        for _ in 0..3 {
            monitor.start_timer("test_op");
            thread::sleep(Duration::from_millis(10));
            monitor.stop_timer("test_op");
        }
        
        let avg = monitor.get_average_duration("test_op");
        assert!(avg.is_some());
        assert!(avg.unwrap() >= 10);
    }
    
    #[test]
    fn test_scoped_timer() {
        let monitor = PerformanceMonitor::new(true);
        
        {
            let _timer = ScopedTimer::new(&monitor, "scoped_op");
            thread::sleep(Duration::from_millis(10));
        }
        
        let metrics = monitor.get_operation_metrics("scoped_op");
        assert_eq!(metrics.len(), 1);
        assert!(metrics[0].duration_ms >= 10);
    }
    
    #[test]
    fn test_summary() {
        let monitor = PerformanceMonitor::new(true);
        
        monitor.start_timer("op1");
        thread::sleep(Duration::from_millis(10));
        monitor.stop_timer("op1");
        
        monitor.start_timer("op2");
        thread::sleep(Duration::from_millis(20));
        monitor.stop_timer("op2");
        
        let summary = monitor.get_summary();
        assert_eq!(summary.operations.len(), 2);
    }
}

// Made with Bob
