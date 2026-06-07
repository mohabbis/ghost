//! Integration tests for Ghost application
//! Tests the full workflow from recording to replay

use ghost_lib::core::events::{InputEvent, KeyAction};
use ghost_lib::config::GhostConfig;
use ghost_lib::error::{GhostError, ErrorKind};

#[test]
fn test_config_load_and_save() {
    let config = GhostConfig::default();
    assert!(config.validate().is_ok());
    
    // Test serialization
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: GhostConfig = serde_json::from_str(&json).unwrap();
    
    assert_eq!(config.general.theme, deserialized.general.theme);
    assert_eq!(config.replay.default_speed, deserialized.replay.default_speed);
}

#[test]
fn test_config_validation() {
    let mut config = GhostConfig::default();
    
    // Valid config should pass
    assert!(config.validate().is_ok());
    
    // Invalid speed should fail
    config.replay.default_speed = -1.0;
    assert!(config.validate().is_err());
    
    // Reset and test threshold
    config = GhostConfig::default();
    config.replay.visual_threshold = 1.5;
    assert!(config.validate().is_err());
}

#[test]
fn test_error_creation() {
    let err = GhostError::permission_denied("screen recording");
    assert_eq!(err.kind, ErrorKind::Permission);
    assert!(err.message.contains("Permission denied"));
    assert!(err.suggestion.is_some());
    assert!(!err.code.is_empty());
}

#[test]
fn test_error_code_consistency() {
    let err1 = GhostError::recording_failed("test reason");
    let err2 = GhostError::recording_failed("test reason");
    
    // Same error should generate same code
    assert_eq!(err1.code, err2.code);
}

#[test]
fn test_error_display() {
    let err = GhostError::replay_failed("UI changed")
        .with_suggestion("Try re-recording");
    
    let display = format!("{}", err);
    assert!(display.contains("Failed to replay workflow"));
    assert!(display.contains("UI changed"));
}

#[test]
fn test_input_event_serialization() {
    let event = InputEvent::MouseClick {
        x: 100,
        y: 200,
        button: 1,
        element: None,
        timestamp: Some(12345),
        retry_count: None,
        semantic_tag: None,
        self_heal: Some(true),
    };
    
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: InputEvent = serde_json::from_str(&json).unwrap();
    
    match deserialized {
        InputEvent::MouseClick { x, y, button, .. } => {
            assert_eq!(x, 100);
            assert_eq!(y, 200);
            assert_eq!(button, 1);
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_key_event_serialization() {
    let event = InputEvent::Key {
        code: 65,
        chars: "a".to_string(),
        modifiers: 0,
        action: KeyAction::Down,
        timestamp: Some(12345),
        retry_count: None,
        semantic_tag: None,
    };
    
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: InputEvent = serde_json::from_str(&json).unwrap();
    
    match deserialized {
        InputEvent::Key { code, chars, .. } => {
            assert_eq!(code, 65);
            assert_eq!(chars, "a");
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_workflow_event_sequence() {
    let events = vec![
        InputEvent::MouseClick {
            x: 100,
            y: 100,
            button: 1,
            element: None,
            timestamp: Some(1000),
            retry_count: None,
            semantic_tag: None,
            self_heal: Some(true),
        },
        InputEvent::Delay {
            ms: 500,
            timestamp: Some(1500),
        },
        InputEvent::Key {
            code: 65,
            chars: "a".to_string(),
            modifiers: 0,
            action: KeyAction::Down,
            timestamp: Some(1500),
            retry_count: None,
            semantic_tag: None,
        },
    ];
    
    assert_eq!(events.len(), 3);
    
    // Test serialization of sequence
    let json = serde_json::to_string(&events).unwrap();
    let deserialized: Vec<InputEvent> = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.len(), 3);
}

#[test]
fn test_privacy_settings() {
    let config = GhostConfig::default();
    
    // Default should have privacy-friendly settings
    assert!(config.privacy.anonymize_logs);
    assert!(config.privacy.mask_passwords);
    assert!(!config.privacy.telemetry_enabled); // Opt-in by default
}

#[test]
fn test_performance_settings() {
    let config = GhostConfig::default();
    
    assert!(config.performance.cache_enabled);
    assert!(config.performance.event_buffer_size > 0);
    assert!(config.performance.thread_pool_size > 0);
}

#[test]
fn test_ai_settings() {
    let config = GhostConfig::default();
    
    assert!(config.ai.enabled);
    assert!(config.ai.auto_optimize);
    assert!(config.ai.proactive_suggestions);
}

#[cfg(test)]
mod workflow_tests {
    use super::*;
    
    #[test]
    fn test_empty_workflow() {
        let events: Vec<InputEvent> = vec![];
        let json = serde_json::to_string(&events).unwrap();
        let deserialized: Vec<InputEvent> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 0);
    }
    
    #[test]
    fn test_complex_workflow() {
        let events = vec![
            InputEvent::MouseClick {
                x: 100,
                y: 100,
                button: 1,
                element: None,
                timestamp: Some(1000),
                retry_count: None,
                semantic_tag: None,
                self_heal: Some(true),
            },
            InputEvent::Delay {
                ms: 100,
                timestamp: Some(1100),
            },
            InputEvent::Key {
                code: 65,
                chars: "a".to_string(),
                modifiers: 0,
                action: KeyAction::Down,
                timestamp: Some(1100),
                retry_count: None,
                semantic_tag: None,
            },
            InputEvent::Key {
                code: 65,
                chars: "a".to_string(),
                modifiers: 0,
                action: KeyAction::Up,
                timestamp: Some(1150),
                retry_count: None,
                semantic_tag: None,
            },
            InputEvent::Scroll {
                dx: 0,
                dy: 100,
                phase: 0,
                timestamp: Some(1200),
            },
        ];
        
        assert_eq!(events.len(), 5);
        
        // Verify serialization round-trip
        let json = serde_json::to_string(&events).unwrap();
        let deserialized: Vec<InputEvent> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 5);
    }
}

// Made with Bob
