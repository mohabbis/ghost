//! Centralized error handling for Ghost application
//! Provides user-friendly error messages and proper error propagation

use serde::{Deserialize, Serialize};
use std::fmt;

/// Main error type for Ghost application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostError {
    /// Error category for better handling
    pub kind: ErrorKind,
    /// User-friendly error message
    pub message: String,
    /// Technical details for debugging
    pub details: Option<String>,
    /// Suggested action for the user
    pub suggestion: Option<String>,
    /// Error code for tracking
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorKind {
    /// Permission-related errors (accessibility, file access)
    Permission,
    /// Configuration errors
    Configuration,
    /// Recording errors
    Recording,
    /// Replay errors
    Replay,
    /// File system errors
    FileSystem,
    /// Network errors (cloud sync, API calls)
    Network,
    /// AI/LLM errors
    AI,
    /// Platform-specific errors
    Platform,
    /// Validation errors
    Validation,
    /// Internal errors
    Internal,
}

impl GhostError {
    /// Create a new error with kind and message
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        let msg = message.into();
        let code = Self::generate_code(&kind, &msg);
        
        Self {
            kind,
            message: msg,
            details: None,
            suggestion: None,
            code,
        }
    }
    
    /// Add technical details
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
    
    /// Add user suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
    
    /// Generate error code for tracking
    fn generate_code(kind: &ErrorKind, message: &str) -> String {
        let prefix = match kind {
            ErrorKind::Permission => "PERM",
            ErrorKind::Configuration => "CONF",
            ErrorKind::Recording => "REC",
            ErrorKind::Replay => "REP",
            ErrorKind::FileSystem => "FS",
            ErrorKind::Network => "NET",
            ErrorKind::AI => "AI",
            ErrorKind::Platform => "PLAT",
            ErrorKind::Validation => "VAL",
            ErrorKind::Internal => "INT",
        };
        
        // Simple hash of message for unique code
        let hash = message.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));
        format!("{}-{:04X}", prefix, hash % 0xFFFF)
    }
    
    // Common error constructors
    
    pub fn permission_denied(resource: &str) -> Self {
        Self::new(
            ErrorKind::Permission,
            format!("Permission denied to access {}", resource)
        )
        .with_suggestion("Please grant the required permissions in System Settings")
    }
    
    pub fn accessibility_required() -> Self {
        Self::new(
            ErrorKind::Permission,
            "Accessibility permission is required"
        )
        .with_suggestion("Go to System Settings → Privacy & Security → Accessibility and enable Ghost")
    }
    
    pub fn recording_failed(reason: &str) -> Self {
        Self::new(
            ErrorKind::Recording,
            "Failed to start recording"
        )
        .with_details(reason)
        .with_suggestion("Check that Ghost has accessibility permissions and try again")
    }
    
    pub fn replay_failed(reason: &str) -> Self {
        Self::new(
            ErrorKind::Replay,
            "Failed to replay workflow"
        )
        .with_details(reason)
        .with_suggestion("The UI may have changed. Try re-recording the workflow")
    }
    
    pub fn file_not_found(path: &str) -> Self {
        Self::new(
            ErrorKind::FileSystem,
            format!("File not found: {}", path)
        )
        .with_suggestion("Check that the file exists and you have permission to access it")
    }
    
    pub fn invalid_workflow(reason: &str) -> Self {
        Self::new(
            ErrorKind::Validation,
            "Invalid workflow"
        )
        .with_details(reason)
        .with_suggestion("The workflow file may be corrupted. Try re-recording it")
    }
    
    pub fn network_error(reason: &str) -> Self {
        Self::new(
            ErrorKind::Network,
            "Network request failed"
        )
        .with_details(reason)
        .with_suggestion("Check your internet connection and try again")
    }
    
    pub fn ai_error(reason: &str) -> Self {
        Self::new(
            ErrorKind::AI,
            "AI operation failed"
        )
        .with_details(reason)
        .with_suggestion("Check your AI provider configuration and API key")
    }
    
    pub fn config_error(reason: &str) -> Self {
        Self::new(
            ErrorKind::Configuration,
            "Configuration error"
        )
        .with_details(reason)
        .with_suggestion("Check your configuration file or reset to defaults")
    }
}

impl fmt::Display for GhostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if let Some(details) = &self.details {
            write!(f, " ({})", details)?;
        }
        Ok(())
    }
}

impl std::error::Error for GhostError {}

// Conversions from common error types
impl From<std::io::Error> for GhostError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => {
                Self::new(ErrorKind::FileSystem, "File not found")
                    .with_details(err.to_string())
            }
            std::io::ErrorKind::PermissionDenied => {
                Self::permission_denied("file system")
                    .with_details(err.to_string())
            }
            _ => {
                Self::new(ErrorKind::FileSystem, "File system error")
                    .with_details(err.to_string())
            }
        }
    }
}

impl From<serde_json::Error> for GhostError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(ErrorKind::Validation, "JSON parsing error")
            .with_details(err.to_string())
            .with_suggestion("The file may be corrupted or in an invalid format")
    }
}

impl From<anyhow::Error> for GhostError {
    fn from(err: anyhow::Error) -> Self {
        Self::new(ErrorKind::Internal, "Internal error")
            .with_details(err.to_string())
    }
}

/// Result type alias for Ghost operations
pub type GhostResult<T> = Result<T, GhostError>;

/// Extension trait for Result to add context
pub trait ResultExt<T> {
    fn context(self, message: impl Into<String>) -> GhostResult<T>;
    fn with_suggestion(self, suggestion: impl Into<String>) -> GhostResult<T>;
}

impl<T, E: Into<GhostError>> ResultExt<T> for Result<T, E> {
    fn context(self, message: impl Into<String>) -> GhostResult<T> {
        self.map_err(|e| {
            let mut err: GhostError = e.into();
            err.details = Some(message.into());
            err
        })
    }
    
    fn with_suggestion(self, suggestion: impl Into<String>) -> GhostResult<T> {
        self.map_err(|e| {
            let mut err: GhostError = e.into();
            err.suggestion = Some(suggestion.into());
            err
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation() {
        let err = GhostError::permission_denied("screen recording");
        assert_eq!(err.kind, ErrorKind::Permission);
        assert!(err.suggestion.is_some());
    }
    
    #[test]
    fn test_error_code_generation() {
        let err1 = GhostError::new(ErrorKind::Recording, "test");
        let err2 = GhostError::new(ErrorKind::Recording, "test");
        assert_eq!(err1.code, err2.code);
    }
    
    #[test]
    fn test_error_display() {
        let err = GhostError::new(ErrorKind::Replay, "Test error")
            .with_details("Some details");
        let display = format!("{}", err);
        assert!(display.contains("Test error"));
        assert!(display.contains("Some details"));
    }
}

// Made with Bob
