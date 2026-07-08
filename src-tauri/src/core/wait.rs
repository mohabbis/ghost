//! Smart wait conditions for workflow execution.
//! Provides polling-based waiting for UI elements, text, or images.

use crate::core::events::{ElementSelector, VarType, WaitCondition};
use crate::core::traits::ElementLocator;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(test)]
thread_local! {
    pub static MOCK_OCR_RESULTS: std::cell::RefCell<Option<Vec<crate::core::ocr::OcrResult>>> = std::cell::RefCell::new(None);
}

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
    pub fn resolve(&mut self, _name: &str, var_type: &VarType) -> anyhow::Result<String> {
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
                const CHARSET: &[u8] =
                    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
                use std::time::{SystemTime, UNIX_EPOCH};
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as usize;
                let s: String = (0..*length)
                    .map(|i| {
                        let idx = (nanos + i * 7) % CHARSET.len();
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
                // Confine CSV reads to the ghost data directory. Without this,
                // a workflow could point `path` at /etc/passwd, ~/.ssh/id_rsa,
                // etc. and exfiltrate it into a replayed keystroke.
                let safe_path = sanitized_csv_path(path)?;
                let contents = std::fs::read_to_string(&safe_path)
                    .map_err(|e| anyhow::anyhow!("Cannot read CSV '{}': {}", path, e))?;
                // Enforce a size cap and basic structural validation.
                crate::core::security::validate_csv_contents(&contents)?;
                let mut lines = contents.lines();
                // First line is the header
                let headers: Vec<&str> = lines
                    .next()
                    .unwrap_or("")
                    .split(',')
                    .map(|s| s.trim())
                    .collect();
                let col_idx = headers.iter().position(|h| h == column).ok_or_else(|| {
                    anyhow::anyhow!(
                        "CSV column '{}' not found (available columns: {})",
                        column,
                        headers.join(", ")
                    )
                })?;
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
                // Only allow explicitly opted-in env vars. Without this guard a
                // workflow could read OPENAI_API_KEY / AWS_SECRET_ACCESS_KEY /
                // etc. and type the secret during replay.
                if !is_allowed_env_key(key) {
                    anyhow::bail!(
                        "ENV var '{}' is not allowed; only GHOST_-prefixed, non-secret vars can be used",
                        key
                    );
                }
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

/// Validate and confine a CSV path to the ghost data directory.
///
/// Requires a `.csv` extension (and no `..` segments) via
/// [`crate::core::security::validate_csv_path`], then confines the resolved path
/// under `<data_dir>/ghost` via [`crate::core::security::sanitize_file_path`].
fn sanitized_csv_path(path: &str) -> anyhow::Result<std::path::PathBuf> {
    crate::core::security::validate_csv_path(path)?;
    let base = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
        .join("ghost");
    crate::core::security::sanitize_file_path(path, &base)
}

/// Whether an environment variable may be resolved by a workflow.
///
/// Only `GHOST_`-prefixed variables are allowed, and any name that looks like it
/// holds a secret is rejected outright as defense-in-depth.
fn is_allowed_env_key(key: &str) -> bool {
    if !key.starts_with("GHOST_") {
        return false;
    }
    let upper = key.to_uppercase();
    const SECRET_MARKERS: [&str; 5] = ["KEY", "TOKEN", "SECRET", "PASSWORD", "CREDENTIAL"];
    !SECRET_MARKERS.iter().any(|m| upper.contains(m))
}

/// Check if a wait condition is satisfied
pub fn check_wait_condition(condition: &WaitCondition, locator: &dyn ElementLocator) -> WaitResult {
    match condition {
        WaitCondition::ElementVisible { selector } => match resolve_selector(selector, locator) {
            Ok((x, y)) => match locator.inspect_at(x, y) {
                Ok(Some(_)) => WaitResult::Success,
                Ok(None) => WaitResult::Timeout,
                Err(e) => WaitResult::Error(e.to_string()),
            },
            Err(e) => WaitResult::Error(e.to_string()),
        },
        WaitCondition::ElementExists { selector } => match resolve_selector(selector, locator) {
            Ok((x, y)) => match locator.inspect_at(x, y) {
                Ok(Some(_)) => WaitResult::Success,
                Ok(None) => WaitResult::Timeout,
                Err(e) => WaitResult::Error(e.to_string()),
            },
            Err(e) => WaitResult::Error(e.to_string()),
        },
        WaitCondition::TextPresent { text } => {
            // Search for element containing the text via accessibility API,
            // bounded by the real display size rather than a fixed 1000×1000.
            let (max_x, max_y) = crate::core::vision::display_bounds();
            for y in (0..max_y).step_by(48) {
                for x in (0..max_x).step_by(48) {
                    if let Ok(Some(el)) = locator.inspect_at(x, y) {
                        if el.name.to_lowercase().contains(&text.to_lowercase()) {
                            return WaitResult::Success;
                        }
                    }
                }
            }
            WaitResult::Timeout
        }
        WaitCondition::ImageMatches {
            baseline,
            threshold,
        } => {
            use crate::core::vision;

            // Capture current screen and compare to baseline
            match vision::capture_screenshot() {
                Ok(img_bytes) => match image::load_from_memory(&img_bytes) {
                    Ok(current_img) => match vision::compare_images(baseline, &current_img) {
                        Ok(similarity) => {
                            if similarity >= *threshold {
                                WaitResult::Success
                            } else {
                                WaitResult::Timeout
                            }
                        }
                        Err(e) => WaitResult::Error(e.to_string()),
                    },
                    Err(e) => WaitResult::Error(e.to_string()),
                },
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
            // Probe a coarse grid: UI elements are tens of pixels wide, so a
            // 48px stride finds them with ~900 lookups instead of the 1M a
            // per-pixel scan would need (which took minutes per poll). The grid
            // is bounded by the real display size so larger screens are covered.
            const STRIDE: i32 = 48;
            let (max_x, max_y) = crate::core::vision::display_bounds();

            let mut found: Option<(i32, i32)> = None;

            'scan: for y in (0..max_y).step_by(STRIDE as usize) {
                for x in (0..max_x).step_by(STRIDE as usize) {
                    if let Ok(Some(el)) = locator.inspect_at(x, y) {
                        if el.role == *role
                            && el.name.contains(name)
                            && app.as_ref().is_none_or(|a| &el.app == a)
                        {
                            found = Some((x, y));
                            break 'scan;
                        }
                    }
                }
            }

            found.ok_or_else(|| anyhow::anyhow!("Element not found: {} {:?}", name, role))
        }
        ElementSelector::OCR { text, fuzzy } => {
            let ocr_results = {
                #[cfg(test)]
                {
                    MOCK_OCR_RESULTS.with(|m| m.borrow().clone())
                }
                #[cfg(not(test))]
                None
            };

            let ocr_results = match ocr_results {
                Some(results) => results,
                None => {
                    let screenshot_bytes = crate::core::vision::capture_screenshot()?;
                    crate::core::ocr::run_ocr(&screenshot_bytes)?
                }
            };

            let matched_result = ocr_results.iter().find(|res| {
                if *fuzzy {
                    res.text.trim().to_lowercase().contains(&text.trim().to_lowercase())
                } else {
                    res.text.trim() == text.trim()
                }
            });

            match matched_result {
                Some(res) => {
                    let (display_w, display_h) = crate::core::vision::display_bounds();
                    #[cfg(target_os = "macos")]
                    let center_y = (1.0 - (res.y + res.h / 2.0)) * display_h as f32;

                    #[cfg(not(target_os = "macos"))]
                    let center_y = (res.y + res.h / 2.0) * display_h as f32;

                    let center_x = (res.x + res.w / 2.0) * display_w as f32;

                    Ok((center_x.round() as i32, center_y.round() as i32))
                }
                None => {
                    let all_text: Vec<String> = ocr_results.iter().map(|res| res.text.clone()).collect();
                    anyhow::bail!(
                        "OCR selector target '{}' (fuzzy: {}) not found on screen. Recognized text: {:?}",
                        text, fuzzy, all_text
                    );
                }
            }
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
        WaitResult::Timeout => Err(anyhow::anyhow!(
            "Wait condition timed out after {}ms",
            timeout_ms
        )),
        WaitResult::Error(e) => Err(anyhow::anyhow!("Wait error: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::ElementInfo;
    use std::collections::HashMap;

    /// A locator stub that returns pre-configured elements at exact coordinates,
    /// and `None` everywhere else (mirrors the headless backend's behavior).
    struct MockLocator {
        elements: HashMap<(i32, i32), ElementInfo>,
    }

    impl MockLocator {
        fn empty() -> Self {
            Self {
                elements: HashMap::new(),
            }
        }

        fn with_element(x: i32, y: i32, element: ElementInfo) -> Self {
            let mut elements = HashMap::new();
            elements.insert((x, y), element);
            Self { elements }
        }
    }

    impl ElementLocator for MockLocator {
        fn inspect_at(&self, x: i32, y: i32) -> anyhow::Result<Option<ElementInfo>> {
            Ok(self.elements.get(&(x, y)).cloned())
        }
    }

    fn sample_element(role: &str, name: &str, app: &str) -> ElementInfo {
        ElementInfo {
            role: role.to_string(),
            name: name.to_string(),
            app: app.to_string(),
            ..Default::default()
        }
    }

    // --- VariableContext ---

    #[test]
    fn variable_context_get_set() {
        let mut ctx = VariableContext::new();
        assert_eq!(ctx.get("missing"), None);

        ctx.set("name".to_string(), "Ghost".to_string());
        assert_eq!(ctx.get("name"), Some(&"Ghost".to_string()));
    }

    #[test]
    fn resolve_random_email_has_expected_format() {
        let mut ctx = VariableContext::new();
        let email = ctx.resolve("e", &VarType::RandomEmail).unwrap();
        assert!(email.starts_with("user"));
        assert!(email.ends_with("@example.com"));
    }

    #[test]
    fn resolve_random_string_has_requested_length() {
        let mut ctx = VariableContext::new();
        let s = ctx
            .resolve("s", &VarType::RandomString { length: 12 })
            .unwrap();
        assert_eq!(s.len(), 12);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn resolve_timestamp_is_numeric() {
        let mut ctx = VariableContext::new();
        let ts = ctx.resolve("t", &VarType::Timestamp).unwrap();
        assert!(!ts.is_empty());
        assert!(ts.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn resolve_from_env_returns_value() {
        let mut ctx = VariableContext::new();
        std::env::set_var("GHOST_TEST_VAR", "hello");
        let val = ctx
            .resolve(
                "v",
                &VarType::FromEnv {
                    key: "GHOST_TEST_VAR".to_string(),
                },
            )
            .unwrap();
        assert_eq!(val, "hello");
        std::env::remove_var("GHOST_TEST_VAR");
    }

    #[test]
    fn resolve_from_env_missing_key_errors() {
        let mut ctx = VariableContext::new();
        let result = ctx.resolve(
            "v",
            &VarType::FromEnv {
                key: "GHOST_TEST_VAR_DOES_NOT_EXIST".to_string(),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn resolve_from_env_rejects_non_ghost_prefix() {
        // Even if the var exists, a non-GHOST_ key must be refused.
        std::env::set_var("PATH_LIKE_LEAK", "anything");
        let mut ctx = VariableContext::new();
        let result = ctx.resolve(
            "v",
            &VarType::FromEnv {
                key: "PATH_LIKE_LEAK".to_string(),
            },
        );
        assert!(result.is_err());
        std::env::remove_var("PATH_LIKE_LEAK");
    }

    #[test]
    fn resolve_from_env_rejects_secret_looking_keys() {
        std::env::set_var("GHOST_OPENAI_API_KEY", "sk-secret");
        let mut ctx = VariableContext::new();
        let result = ctx.resolve(
            "v",
            &VarType::FromEnv {
                key: "GHOST_OPENAI_API_KEY".to_string(),
            },
        );
        assert!(result.is_err());
        std::env::remove_var("GHOST_OPENAI_API_KEY");
    }

    #[test]
    fn is_allowed_env_key_logic() {
        assert!(is_allowed_env_key("GHOST_USERNAME"));
        assert!(is_allowed_env_key("GHOST_REGION"));
        assert!(!is_allowed_env_key("OPENAI_API_KEY"));
        assert!(!is_allowed_env_key("HOME"));
        assert!(!is_allowed_env_key("GHOST_API_KEY"));
        assert!(!is_allowed_env_key("GHOST_AUTH_TOKEN"));
        assert!(!is_allowed_env_key("GHOST_DB_PASSWORD"));
    }

    /// Write a CSV under the confined ghost data dir and return its path.
    fn write_confined_csv(file_name: &str, contents: &str) -> std::path::PathBuf {
        let base = dirs::data_dir().unwrap().join("ghost");
        std::fs::create_dir_all(&base).unwrap();
        let path = base.join(file_name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn resolve_from_csv_reads_column_by_header() {
        let path = write_confined_csv(
            &format!("ghost_wait_test_{}.csv", std::process::id()),
            "name,email\nAlice,alice@example.com\nBob,bob@example.com\n",
        );

        let mut ctx = VariableContext::new();
        let value = ctx
            .resolve(
                "v",
                &VarType::FromCSV {
                    path: path.to_string_lossy().to_string(),
                    column: "email".to_string(),
                    row: Some(1),
                },
            )
            .unwrap();
        assert_eq!(value, "bob@example.com");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn resolve_from_csv_defaults_row_to_zero() {
        let path = write_confined_csv(
            &format!("ghost_wait_test_default_row_{}.csv", std::process::id()),
            "name,email\nAlice,alice@example.com\nBob,bob@example.com\n",
        );

        let mut ctx = VariableContext::new();
        let value = ctx
            .resolve(
                "v",
                &VarType::FromCSV {
                    path: path.to_string_lossy().to_string(),
                    column: "name".to_string(),
                    row: None,
                },
            )
            .unwrap();
        assert_eq!(value, "Alice");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn resolve_from_csv_errors_on_missing_column() {
        let path = write_confined_csv(
            &format!("ghost_wait_test_missing_col_{}.csv", std::process::id()),
            "name,email\nAlice,alice@example.com\n",
        );

        let mut ctx = VariableContext::new();
        let result = ctx.resolve(
            "v",
            &VarType::FromCSV {
                path: path.to_string_lossy().to_string(),
                column: "phone".to_string(), // not in the header
                row: Some(0),
            },
        );
        std::fs::remove_file(&path).ok();

        // Previously this silently fell back to column 0 ("name"); it must now error.
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("phone"),
            "error should name the missing column: {err}"
        );
    }

    #[test]
    fn resolve_from_csv_rejects_path_outside_base() {
        let mut ctx = VariableContext::new();
        // Absolute path with a .csv extension but outside the ghost data dir.
        let result = ctx.resolve(
            "v",
            &VarType::FromCSV {
                path: "/etc/passwd.csv".to_string(),
                column: "x".to_string(),
                row: None,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn resolve_from_csv_rejects_non_csv_extension() {
        let mut ctx = VariableContext::new();
        let result = ctx.resolve(
            "v",
            &VarType::FromCSV {
                path: "data.txt".to_string(),
                column: "x".to_string(),
                row: None,
            },
        );
        assert!(result.is_err());
    }

    // --- resolve_selector ---

    #[test]
    fn resolve_selector_coordinates_is_passthrough() {
        let locator = MockLocator::empty();
        let selector = ElementSelector::Coordinates { x: 42, y: 99 };
        let result = resolve_selector(&selector, &locator).unwrap();
        assert_eq!(result, (42, 99));
    }

    #[test]
    fn resolve_selector_semantic_finds_matching_element() {
        let locator =
            MockLocator::with_element(48, 0, sample_element("button", "Submit", "TestApp"));
        let selector = ElementSelector::Semantic {
            role: "button".to_string(),
            name: "Submit".to_string(),
            app: Some("TestApp".to_string()),
        };
        let result = resolve_selector(&selector, &locator).unwrap();
        assert_eq!(result, (48, 0));
    }

    #[test]
    fn resolve_selector_semantic_not_found_errors() {
        let locator = MockLocator::empty();
        let selector = ElementSelector::Semantic {
            role: "button".to_string(),
            name: "Nonexistent".to_string(),
            app: None,
        };
        assert!(resolve_selector(&selector, &locator).is_err());
    }

    #[test]
    fn resolve_selector_ocr_exact_match_success() {
        let locator = MockLocator::empty();
        
        let mock_results = vec![
            crate::core::ocr::OcrResult {
                text: "Login Button".to_string(),
                x: 0.1,
                y: 0.2,
                w: 0.1,
                h: 0.05,
            },
            crate::core::ocr::OcrResult {
                text: "Cancel".to_string(),
                x: 0.3,
                y: 0.4,
                w: 0.05,
                h: 0.02,
            },
        ];
        
        MOCK_OCR_RESULTS.with(|m| *m.borrow_mut() = Some(mock_results));
        
        let selector = ElementSelector::OCR {
            text: "Cancel".to_string(),
            fuzzy: false,
        };
        
        let (display_w, display_h) = crate::core::vision::display_bounds();
        
        #[cfg(target_os = "macos")]
        let expected_y = ((1.0 - (0.4 + 0.02 / 2.0)) * display_h as f32).round() as i32;
        #[cfg(not(target_os = "macos"))]
        let expected_y = ((0.4 + 0.02 / 2.0) * display_h as f32).round() as i32;
        
        let expected_x = ((0.3 + 0.05 / 2.0) * display_w as f32).round() as i32;
        
        let result = resolve_selector(&selector, &locator).unwrap();
        assert_eq!(result, (expected_x, expected_y));
        
        MOCK_OCR_RESULTS.with(|m| *m.borrow_mut() = None);
    }

    #[test]
    fn resolve_selector_ocr_fuzzy_match_success() {
        let locator = MockLocator::empty();
        
        let mock_results = vec![
            crate::core::ocr::OcrResult {
                text: "Submit Query".to_string(),
                x: 0.5,
                y: 0.5,
                w: 0.2,
                h: 0.1,
            },
        ];
        
        MOCK_OCR_RESULTS.with(|m| *m.borrow_mut() = Some(mock_results));
        
        let selector = ElementSelector::OCR {
            text: "submit".to_string(),
            fuzzy: true,
        };
        
        let (display_w, display_h) = crate::core::vision::display_bounds();
        let expected_x = ((0.5 + 0.2 / 2.0) * display_w as f32).round() as i32;
        #[cfg(target_os = "macos")]
        let expected_y = ((1.0 - (0.5 + 0.1 / 2.0)) * display_h as f32).round() as i32;
        #[cfg(not(target_os = "macos"))]
        let expected_y = ((0.5 + 0.1 / 2.0) * display_h as f32).round() as i32;
        
        let result = resolve_selector(&selector, &locator).unwrap();
        assert_eq!(result, (expected_x, expected_y));
        
        MOCK_OCR_RESULTS.with(|m| *m.borrow_mut() = None);
    }

    #[test]
    fn resolve_selector_ocr_not_found_errors_actionably() {
        let locator = MockLocator::empty();
        
        let mock_results = vec![
            crate::core::ocr::OcrResult {
                text: "Open File".to_string(),
                x: 0.1,
                y: 0.1,
                w: 0.1,
                h: 0.1,
            },
        ];
        
        MOCK_OCR_RESULTS.with(|m| *m.borrow_mut() = Some(mock_results));
        
        let selector = ElementSelector::OCR {
            text: "Save".to_string(),
            fuzzy: false,
        };
        
        let result = resolve_selector(&selector, &locator);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Save"));
        assert!(err_msg.contains("Open File"));
        
        MOCK_OCR_RESULTS.with(|m| *m.borrow_mut() = None);
    }

    // --- check_wait_condition / wait_for_condition / smart_wait ---

    #[test]
    fn check_element_visible_success_and_timeout() {
        let found = MockLocator::with_element(10, 20, sample_element("button", "OK", "App"));
        let visible = WaitCondition::ElementVisible {
            selector: ElementSelector::Coordinates { x: 10, y: 20 },
        };
        assert!(matches!(
            check_wait_condition(&visible, &found),
            WaitResult::Success
        ));

        let empty = MockLocator::empty();
        assert!(matches!(
            check_wait_condition(&visible, &empty),
            WaitResult::Timeout
        ));
    }

    #[test]
    fn check_text_present_finds_matching_element_name() {
        let locator =
            MockLocator::with_element(0, 0, sample_element("label", "Welcome Home", "App"));
        let condition = WaitCondition::TextPresent {
            text: "welcome".to_string(),
        };
        assert!(matches!(
            check_wait_condition(&condition, &locator),
            WaitResult::Success
        ));
    }

    #[test]
    fn check_text_present_not_found_times_out() {
        let locator = MockLocator::empty();
        let condition = WaitCondition::TextPresent {
            text: "nope".to_string(),
        };
        assert!(matches!(
            check_wait_condition(&condition, &locator),
            WaitResult::Timeout
        ));
    }

    #[test]
    fn check_custom_condition_always_succeeds() {
        let locator = MockLocator::empty();
        let condition = WaitCondition::Custom {
            js_expression: "true".to_string(),
        };
        assert!(matches!(
            check_wait_condition(&condition, &locator),
            WaitResult::Success
        ));
    }

    #[test]
    fn wait_for_condition_times_out_quickly() {
        let locator = MockLocator::empty();
        let condition = WaitCondition::ElementExists {
            selector: ElementSelector::Coordinates { x: 0, y: 0 },
        };
        let result = wait_for_condition(&condition, &locator, 50, 10);
        assert!(matches!(result, WaitResult::Timeout));
    }

    #[test]
    fn smart_wait_ok_and_err_mapping() {
        let found = MockLocator::with_element(0, 0, sample_element("button", "OK", "App"));
        let visible = WaitCondition::ElementVisible {
            selector: ElementSelector::Coordinates { x: 0, y: 0 },
        };
        assert!(smart_wait(&visible, &found, 100, 10).is_ok());

        let empty = MockLocator::empty();
        let err = smart_wait(&visible, &empty, 30, 10);
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("timed out"));
    }
}
