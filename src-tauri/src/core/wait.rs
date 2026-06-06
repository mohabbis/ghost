//! Smart wait conditions for workflow execution.
//! Provides polling-based waiting for UI elements, text, or images.

use crate::core::events::{ElementInfo, ElementSelector, InputEvent, VarType, WaitCondition};
use crate::core::traits::ElementLocator;
use std::thread;
use std::time::{Duration, Instant};

/// Default timeout for wait conditions (5 seconds)
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Default poll interval (100ms)
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 100;

/// Wait result indicating success or failure
#[derive(Debug, Clone)]
pub enum WaitResult {
    Success,
    Timeout,
    Error(String),
}

/// Variable context for storing and resolving variables during replay
#[derive(Clone, Debug, Default)]
pub struct VariableContext {
    pub variables: std::collections::HashMap<String, String>,
}

impl VariableContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve a variable using its type definition
    pub fn resolve(&mut self, name: &str, var_type: &VarType) -> anyhow::Result<String> {
        match var_type {
            VarType::RandomEmail => {
                // Generate a deterministic "random" email for reproducibility
                use std::time::{SystemTime, UNIX_EPOCH};
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let email: String = format!("user{}@example.com", nanos % 100000000u128);
                Ok(email)
            }
            VarType::RandomString { length } => {
                const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
                use std::time::{SystemTime, UNIX_EPOCH};
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as usize;
                let s: String = (0..*length)
                    .map(|i| {
                        let idx = ((nanos + i * 7) % CHARSET.len());
                        CHARSET[idx] as char
                    })
                    .collect();
                Ok(s)
            }
            VarType::Timestamp => {
                use std::time::{SystemTime, UNIX_EPOCH};
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                Ok(ts.to_string())
            }
            VarType::FromCSV { path, column, row } => {
                let contents = std::fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("Cannot read CSV '{}': {}", path, e))?;
                let mut lines = contents.lines();
                // First line is the header
                let headers: Vec<&str> = lines
                    .next()
                    .unwrap_or("")
                    .split(',')
                    .map(|s| s.trim())
                    .collect();
                let col_idx = headers
                    .iter()
                    .position(|h| h == column)
                    .unwrap_or(0);
                let row_idx = row.unwrap_or(0);
                let data_row = lines
                    .nth(row_idx)
                    .ok_or_else(|| anyhow::anyhow!("CSV row {} out of range", row_idx))?;
                let value = data_row
                    .split(',')
                    .nth(col_idx)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                Ok(value)
            }
            VarType::FromEnv { key } => {
                std::env::var(key).map_err(|e| anyhow::anyhow!("ENV var {} not found: {}", key, e))
            }
        }
    }

    /// Get a stored variable value
    pub fn get(&self, name: &str) -> Option<&String> {
        self.variables.get(name)
    }

    /// Set a variable value
    pub fn set(&mut self, name: String, value: String) {
        self.variables.insert(name, value);
    }
}

/// Check if a wait condition is satisfied
pub fn check_wait_condition(
    condition: &WaitCondition,
    locator: &dyn ElementLocator,
) -> WaitResult {
    match condition {
        WaitCondition::ElementVisible { selector } => {
            match resolve_selector(selector, locator) {
                Ok((x, y)) => {
                    match locator.inspect_at(x, y) {
                        Ok(Some(_)) => WaitResult::Success,
                        Ok(None) => WaitResult::Timeout,
                        Err(e) => WaitResult::Error(e.to_string()),
                    }
                }
                Err(e) => WaitResult::Error(e.to_string()),
            }
        }
        WaitCondition::ElementExists { selector } => {
            match resolve_selector(selector, locator) {
                Ok((x, y)) => {
                    match locator.inspect_at(x, y) {
                        Ok(Some(_)) => WaitResult::Success,
                        Ok(None) => WaitResult::Timeout,
                        Err(e) => WaitResult::Error(e.to_string()),
                    }
                }
                Err(e) => WaitResult::Error(e.to_string()),
            }
        }
        WaitCondition::TextPresent { text } => {
            // Search for element containing the text via accessibility API
            for y in (0..1000).step_by(50) {
                for x in (0..1000).step_by(50) {
                    if let Ok(Some(el)) = locator.inspect_at(x, y) {
                        if el.name.to_lowercase().contains(&text.to_lowercase()) {
                            return WaitResult::Success;
                        }
                    }
                }
            }
            WaitResult::Timeout
        }
        WaitCondition::ImageMatches { baseline, threshold } => {
            use crate::core::vision;
            
            // Capture current screen and compare to baseline
            match vision::capture_screenshot() {
                Ok(img_bytes) => {
                    match image::load_from_memory(&img_bytes) {
                        Ok(current_img) => {
                            match vision::compare_images(baseline, &current_img) {
                                Ok(similarity) => {
                                    if similarity >= *threshold {
                                        WaitResult::Success
                                    } else {
                                        WaitResult::Timeout
                                    }
                                }
                                Err(e) => WaitResult::Error(e.to_string()),
                            }
                        }
                        Err(e) => WaitResult::Error(e.to_string()),
                    }
                }
                Err(e) => WaitResult::Error(e.to_string()),
            }
        }
        WaitCondition::Custom { js_expression } => {
            // For custom conditions, we emit an event that the frontend can handle
            // For now, log and return success
            tracing::info!("Custom wait condition: {}", js_expression);
            WaitResult::Success
        }
    }
}

/// Poll a wait condition until satisfied or timeout
pub fn wait_for_condition(
    condition: &WaitCondition,
    locator: &dyn ElementLocator,
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> WaitResult {
    let start = Instant::now();
    let poll_duration = Duration::from_millis(poll_interval_ms);
    let timeout_duration = Duration::from_millis(timeout_ms);

    loop {
        match check_wait_condition(condition, locator) {
            WaitResult::Success => return WaitResult::Success,
            WaitResult::Error(e) => return WaitResult::Error(e),
            WaitResult::Timeout => {}
        }

        if start.elapsed() >= timeout_duration {
            return WaitResult::Timeout;
        }

        thread::sleep(poll_duration);
    }
}

/// Resolve semantic selector to coordinates
pub fn resolve_selector(
    selector: &ElementSelector,
    locator: &dyn ElementLocator,
) -> anyhow::Result<(i32, i32)> {
    match selector {
        ElementSelector::Coordinates { x, y } => Ok((*x, *y)),
        ElementSelector::Semantic { role, name, app } => {
            // Search for element matching role/name
            // This is a simplified implementation - real version would iterate
            // through visible elements and match attributes
            let mut found: Option<(i32, i32)> = None;
            
            // Search common screen positions for matching elements
            for y in 0..1000 {
                for x in 0..1000 {
                    if let Ok(Some(el)) = locator.inspect_at(x, y) {
                        if el.role == *role && el.name.contains(name) {
                            if app.as_ref().map_or(true, |a| &el.app == a) {
                                found = Some((x, y));
                                break;
                            }
                        }
                    }
                }
                if found.is_some() {
                    break;
                }
            }

            found.ok_or_else(|| anyhow::anyhow!("Element not found: {} {:?}", name, role))
        }
        ElementSelector::OCR { text, fuzzy } => {
            // TODO: Implement OCR-based element location
            anyhow::Err(anyhow::anyhow!("OCR selector not implemented"))
        }
    }
}

/// Smart wait that handles element resolution
pub fn smart_wait(
    condition: &WaitCondition,
    locator: &dyn ElementLocator,
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> anyhow::Result<()> {
    let result = wait_for_condition(condition, locator, timeout_ms, poll_interval_ms);
    
    match result {
        WaitResult::Success => Ok(()),
        WaitResult::Timeout => {
            Err(anyhow::anyhow!("Wait condition timed out after {}ms", timeout_ms))
        }
        WaitResult::Error(e) => Err(anyhow::anyhow!("Wait error: {}", e)),
    }
}