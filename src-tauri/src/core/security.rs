//! Security module for input validation, path sanitization, and encryption.
//! Production-hardening for the Ghost automation platform.

use std::path::{Path, PathBuf};
use std::ffi::OsStr;

/// Maximum allowed path length to prevent buffer overflows
const MAX_PATH_LENGTH: usize = 4096;

/// Allowed characters for workflow names (alphanumeric, dash, underscore, space)
const WORKFLOW_NAME_PATTERN: &str = r"^[a-zA-Z0-9_\- ]+$";

/// Security audit configuration
pub mod audit {
    use serde::{Deserialize, Serialize};

    /// Security audit finding
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SecurityFinding {
        pub severity: Severity,
        pub category: Category,
        pub message: String,
        pub file: Option<String>,
        pub line: Option<usize>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Severity {
        Low,
        Medium,
        High,
        Critical,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Category {
        PathTraversal,
        InputValidation,
        Encryption,
        AccessControl,
    }

    /// Run security audit on codebase
    pub fn run_audit() -> Vec<SecurityFinding> {
        let mut findings = Vec::new();
        
        // Check for unsafe practices in file operations
        // This would be expanded to scan actual source files
        
        findings
    }
}

/// Path sanitization for workflow files
pub fn sanitize_workflow_path(name: &str) -> anyhow::Result<PathBuf> {
    // Validate name format
    if name.is_empty() || name.len() > 255 {
        anyhow::bail!("Workflow name must be 1-255 characters");
    }

    // Check for path traversal attempts
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        anyhow::bail!("Invalid workflow name: path traversal detected");
    }

    // Check for null bytes
    if name.contains('\0') {
        anyhow::bail!("Invalid workflow name: null byte detected");
    }

    // Validate character set
    if !regex::Regex::new(WORKFLOW_NAME_PATTERN)
        .map(|re| re.is_match(name))
        .unwrap_or(true)
    {
        anyhow::bail!("Workflow name contains invalid characters");
    }

    Ok(PathBuf::from(name))
}

/// Sanitize arbitrary file paths to prevent directory traversal
pub fn sanitize_file_path(path: &str, base_dir: &Path) -> anyhow::Result<PathBuf> {
    if path.len() > MAX_PATH_LENGTH {
        anyhow::bail!("Path exceeds maximum length");
    }

    let cleaned = path.replace('\\', "/");
    
    // Check for null bytes
    if cleaned.contains('\0') {
        anyhow::bail!("Invalid path: null byte detected");
    }

    // Normalize and check if within base directory
    let candidate = base_dir.join(&cleaned);
    let canonical_base = base_dir.canonicalize().unwrap_or_else(|_| base_dir.to_path_buf());
    let canonical_candidate = candidate.canonicalize().unwrap_or_else(|_| candidate.clone());

    if !canonical_candidate.starts_with(&canonical_base) {
        anyhow::bail!("Path traversal attempt blocked");
    }

    Ok(candidate)
}

/// Validate and sanitize screenshot data
pub fn validate_screenshot(data: &[u8]) -> anyhow::Result<()> {
    if data.is_empty() {
        anyhow::bail!("Screenshot data is empty");
    }

    // Maximum size: 50MB
    if data.len() > 50 * 1024 * 1024 {
        anyhow::bail!("Screenshot exceeds maximum size (50MB)");
    }

    // Verify PNG/JPEG magic bytes
    let is_png = data.len() >= 8 && 
        &data[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let is_jpeg = data.len() >= 2 &&
        data[0] == 0xFF && data[1] == 0xD8;

    if !is_png && !is_jpeg {
        anyhow::bail!("Invalid image format: expected PNG or JPEG");
    }

    Ok(())
}

/// Simple XOR encryption for sensitive workflow data
pub struct SimpleCrypto {
    key: [u8; 32],
}

impl SimpleCrypto {
    /// Create a new crypto instance with a key
    pub fn new(key: &str) -> Self {
        let mut key_bytes = [0u8; 32];
        let key_chars = key.as_bytes();
        for (i, byte) in key_bytes.iter_mut().enumerate() {
            *byte = key_chars[i % key_chars.len()].wrapping_add(i as u8);
        }
        Self { key: key_bytes }
    }

    /// Encrypt data (XOR cipher with key rotation)
    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, &b)| b ^ self.key[i % 32])
            .collect()
    }

    /// Decrypt data
    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        // XOR is symmetric, so encrypt and decrypt are the same
        self.encrypt(data)
    }
}

/// Validate CSV file path and contents
pub fn validate_csv_path(path: &str) -> anyhow::Result<PathBuf> {
    let path = Path::new(path);
    
    // Must have .csv extension
    if path.extension() != Some(OsStr::new("csv")) {
        anyhow::bail!("File must have .csv extension");
    }

    // Check for suspicious patterns
    let path_str = path.to_string_lossy();
    if path_str.contains("..") {
        anyhow::bail!("Invalid CSV path: directory traversal detected");
    }

    Ok(path.to_path_buf())
}

/// Validate CSV contents
pub fn validate_csv_contents(contents: &str) -> anyhow::Result<Vec<String>> {
    // Maximum file size: 10MB
    if contents.len() > 10 * 1024 * 1024 {
        anyhow::bail!("CSV contents exceed maximum size");
    }

    // Parse and validate
    let mut headers = Vec::new();
    for (i, line) in contents.lines().enumerate() {
        if i == 0 {
            // Validate headers
            for header in line.split(',') {
                let header = header.trim();
                if header.is_empty() {
                    anyhow::bail!("CSV has empty column header");
                }
                headers.push(header.to_string());
            }
        }
    }

    Ok(headers)
}

/// Input validation for LLM prompts
pub fn validate_prompt(prompt: &str) -> anyhow::Result<()> {
    if prompt.is_empty() {
        anyhow::bail!("Prompt cannot be empty");
    }

    if prompt.len() > 10000 {
        anyhow::bail!("Prompt exceeds maximum length (10000 characters)");
    }

    // Check for potential prompt injection patterns (basic)
    let injection_patterns = ["ignore previous", "disregard", "system:", "assistant:"];
    for pattern in injection_patterns {
        if prompt.to_lowercase().contains(pattern) {
            anyhow::bail!("Potential prompt injection detected");
        }
    }

    Ok(())
}

/// Validate coordinates are within screen bounds
pub fn validate_coordinates(x: i32, y: i32) -> anyhow::Result<()> {
    // Assume max screen size of 10000x10000
    if x < 0 || x > 10000 || y < 0 || y > 10000 {
        anyhow::bail!("Coordinates out of valid range (0-10000)");
    }
    Ok(())
}

/// Rate limiting for API calls
pub mod rate_limit {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    pub struct RateLimiter {
        requests: AtomicU64,
        window_start: AtomicU64,
        max_requests: u64,
        window_duration: Duration,
    }

    impl RateLimiter {
        pub fn new(max_requests: u64, window_duration: Duration) -> Self {
            Self {
                requests: AtomicU64::new(0),
                window_start: AtomicU64::new(
                    Instant::now().duration_since(Instant::UNIX_EPOCH).as_secs()
                ),
                max_requests,
                window_duration,
            }
        }

        pub fn check(&self) -> bool {
            let now = Instant::now().duration_since(Instant::UNIX_EPOCH).as_secs();
            let window_start = self.window_start.load(Ordering::Relaxed);
            
            // Reset window if expired
            if now - window_start > self.window_duration.as_secs() {
                self.window_start.store(now, Ordering::Relaxed);
                self.requests.store(0, Ordering::Relaxed);
            }

            let current = self.requests.load(Ordering::Relaxed);
            if current < self.max_requests {
                self.requests.fetch_add(1, Ordering::Relaxed);
                true
            } else {
                false
            }
        }
    }
}