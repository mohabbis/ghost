//! Integration tests for Ghost application
//! Tests the full workflow from recording to replay

use ghost_lib::config::GhostConfig;
use ghost_lib::core::events::{InputEvent, KeyAction, WORKFLOW_SCHEMA_VERSION};
use ghost_lib::error::{ErrorKind, GhostError};

#[test]
fn test_config_load_and_save() {
    let config = GhostConfig::default();
    assert!(config.validate().is_ok());

    // Test serialization
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: GhostConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.general.theme, deserialized.general.theme);
    assert_eq!(
        config.replay.default_speed,
        deserialized.replay.default_speed
    );
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
    let err = GhostError::replay_failed("UI changed").with_suggestion("Try re-recording");

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

    // AI features are opt-in: Ghost ships privacy-first with every AI toggle
    // off by default, and the provider falls back to local heuristics.
    assert!(!config.ai.enabled);
    assert!(!config.ai.auto_optimize);
    assert!(!config.ai.proactive_suggestions);
    assert_eq!(config.ai.provider, "local");
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

#[test]
fn test_ghost_guard_flags_sensitive_destructive_workflow() {
    use ghost_lib::core::events::ElementInfo;
    use ghost_lib::core::guard::{audit_workflow, GuardCategory, GuardSeverity};

    let events = vec![InputEvent::MouseClick {
        x: 10,
        y: 20,
        button: 0,
        element: Some(ElementInfo {
            role: "AXButton".to_string(),
            name: "Delete password".to_string(),
            app: "System Settings".to_string(),
            fallback_coords: Some((10, 20)),
            value: None,
            description: None,
            identifier: None,
            role_description: Some("button".to_string()),
        }),
        timestamp: Some(1),
        retry_count: None,
        semantic_tag: None,
        self_heal: Some(true),
    }];

    let report = audit_workflow(&events);
    assert!(report.requires_confirmation);
    assert_eq!(report.risk_level, "high");
    assert!(report.score < 65);
    assert!(report
        .sensitive_apps
        .contains(&"System Settings".to_string()));
    // A "Delete password" control matches a secure-field hint, so the guard
    // categorizes it as a SensitiveField (the more specific finding) rather
    // than a generic SensitiveApp, while still recording the app above.
    assert!(report.findings.iter().any(|finding| {
        finding.category == GuardCategory::SensitiveField && finding.severity == GuardSeverity::High
    }));
    assert!(report.findings.iter().any(|finding| {
        finding.category == GuardCategory::DestructiveAction
            && finding.severity == GuardSeverity::High
    }));
}

#[test]
fn test_ghost_guard_clean_workflow_is_low_risk() {
    use ghost_lib::core::events::ElementInfo;
    use ghost_lib::core::guard::audit_workflow;

    let events = vec![InputEvent::MouseClick {
        x: 100,
        y: 200,
        button: 0,
        element: Some(ElementInfo {
            role: "AXButton".to_string(),
            name: "Export".to_string(),
            app: "Numbers".to_string(),
            fallback_coords: Some((100, 200)),
            value: None,
            description: None,
            identifier: Some("export-button".to_string()),
            role_description: Some("button".to_string()),
        }),
        timestamp: Some(1),
        retry_count: None,
        semantic_tag: None,
        self_heal: Some(true),
    }];

    let report = audit_workflow(&events);
    assert!(!report.requires_confirmation);
    assert_eq!(report.risk_level, "low");
    assert_eq!(report.score, 100);
    assert!(report.findings.is_empty());
}

/// Extract the timestamp from any timestamped event variant.
fn event_timestamp(e: &InputEvent) -> Option<u64> {
    match e {
        InputEvent::MouseClick { timestamp, .. } => *timestamp,
        InputEvent::Key { timestamp, .. } => *timestamp,
        InputEvent::Delay { timestamp, .. } => *timestamp,
        InputEvent::Scroll { timestamp, .. } => *timestamp,
        _ => None,
    }
}

/// Full save → load round-trip through GhostEngine. Exercises the workflow
/// persistence path (file IO + name sanitization) end-to-end and asserts that
/// event payloads — crucially the per-event timestamps that drive replay
/// pacing — survive the serialize/deserialize cycle unchanged.
#[test]
fn test_workflow_save_load_round_trip_preserves_timestamps() {
    use ghost_lib::engine::GhostEngine;

    let engine = GhostEngine::new();

    // Unique name keeps the test hermetic against the real data dir / parallel runs.
    let name = format!("itest_roundtrip_{}", std::process::id());

    let events = vec![
        InputEvent::MouseClick {
            x: 320,
            y: 240,
            button: 0,
            element: None,
            timestamp: Some(1000),
            retry_count: None,
            semantic_tag: None,
            self_heal: Some(true),
        },
        InputEvent::MouseClick {
            x: 320,
            y: 240,
            button: 1,
            element: None,
            timestamp: Some(1080),
            retry_count: None,
            semantic_tag: None,
            self_heal: Some(true),
        },
        InputEvent::Delay {
            ms: 500,
            timestamp: Some(1580),
        },
        InputEvent::Key {
            code: 65,
            chars: "a".to_string(),
            modifiers: 0,
            action: KeyAction::Down,
            timestamp: Some(1600),
            retry_count: None,
            semantic_tag: None,
        },
    ];

    let path = engine
        .save_workflow(&name, &events)
        .expect("save should succeed");
    let loaded = engine.load_workflow(&name).expect("load should succeed");

    // Cleanup before asserting so a failure doesn't leave the fixture behind.
    std::fs::remove_file(&path).ok();

    assert_eq!(loaded.len(), events.len());
    let timestamps: Vec<Option<u64>> = loaded.iter().map(event_timestamp).collect();
    assert_eq!(
        timestamps,
        vec![Some(1000), Some(1080), Some(1580), Some(1600)],
        "timestamps must survive the save/load round-trip for replay pacing"
    );

    // The press/release click pair must come back as two distinct events.
    assert!(matches!(
        loaded[0],
        InputEvent::MouseClick { button: 0, .. }
    ));
    assert!(matches!(
        loaded[1],
        InputEvent::MouseClick { button: 1, .. }
    ));
}

#[test]
fn test_workflow_save_writes_schema_envelope() {
    use ghost_lib::engine::GhostEngine;

    let engine = GhostEngine::new();
    let name = format!("itest_schema_{}", std::process::id());
    let events = vec![InputEvent::Delay {
        ms: 250,
        timestamp: Some(250),
    }];

    let path = engine
        .save_workflow(&name, &events)
        .expect("save should succeed");
    let raw = std::fs::read_to_string(&path).expect("saved workflow should be readable");
    std::fs::remove_file(&path).ok();

    let saved: serde_json::Value = serde_json::from_str(&raw).expect("workflow JSON should parse");
    assert_eq!(saved["schema_version"], WORKFLOW_SCHEMA_VERSION);
    assert_eq!(saved["app_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(saved["name"], name);
    assert_eq!(saved["events"].as_array().map(Vec::len), Some(1));
}

#[test]
fn test_legacy_workflow_event_array_still_loads() {
    use ghost_lib::engine::GhostEngine;

    let engine = GhostEngine::new();
    let name = format!("itest_legacy_schema_{}", std::process::id());
    let events = vec![InputEvent::Delay {
        ms: 125,
        timestamp: Some(125),
    }];
    let workflows_dir = dirs::data_dir().unwrap().join("ghost").join("workflows");
    std::fs::create_dir_all(&workflows_dir).expect("workflow dir should be created");
    let path = workflows_dir.join(format!("{name}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&events).unwrap())
        .expect("legacy workflow should be written");

    let loaded = engine
        .load_workflow(&name)
        .expect("legacy load should succeed");
    std::fs::remove_file(&path).ok();

    assert_eq!(loaded.len(), 1);
    assert_eq!(event_timestamp(&loaded[0]), Some(125));
}
