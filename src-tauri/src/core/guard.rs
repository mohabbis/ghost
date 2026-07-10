//! Ghost Guard: local workflow risk audit before replay.
//!
//! The guard is intentionally deterministic and local-first. It does not try to
//! be a cloud AI classifier; it gives users an explainable safety review of the
//! workflow they just recorded.

use crate::core::compression::{CompressedStep, CompressionReport, Target, LOW_CONFIDENCE};
use crate::core::events::{ElementInfo, InputEvent, KeyAction};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GuardSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuardCategory {
    SensitiveApp,
    SensitiveField,
    DestructiveAction,
    CredentialInput,
    LowLocatorConfidence,
    ExternalDraft,
    EmptyWorkflow,
    ReplayReliability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardFinding {
    pub severity: GuardSeverity,
    pub category: GuardCategory,
    pub step_index: Option<usize>,
    pub title: String,
    pub detail: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostGuardReport {
    pub score: u8,
    pub risk_level: String,
    pub event_count: usize,
    pub sensitive_apps: Vec<String>,
    pub findings: Vec<GuardFinding>,
    pub requires_confirmation: bool,
    pub blocks_replay: bool,
    pub safe_to_save: bool,
    pub summary: String,
    pub ai_audit: GuardAuditLayer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardAuditLayer {
    pub privacy_summary: String,
    pub replay_summary: String,
    pub recommended_next_steps: Vec<String>,
    pub blocked_steps: Vec<usize>,
}

const SENSITIVE_APP_HINTS: &[&str] = &[
    "1password",
    "bitwarden",
    "lastpass",
    "keychain",
    "password",
    "bank",
    "wallet",
    "crypto",
    "authenticator",
    "login",
    "sign in",
    "terminal",
    "system settings",
    "settings",
    "outlook",
    "teams",
    "copilot",
    "power bi",
    "fabric",
    "office 365",
    "azure",
];

const DESTRUCTIVE_HINTS: &[&str] = &[
    "delete", "remove", "trash", "send", "submit", "purchase", "pay", "transfer", "install", "run",
    "execute", "confirm",
];

const CREDENTIAL_HINTS: &[&str] = &[
    "password",
    "passcode",
    "token",
    "secret",
    "api key",
    "apikey",
    "otp",
    "verification code",
    "credit card",
    "card number",
    "cvv",
    "ssn",
    "social security",
];

pub fn audit_workflow(events: &[InputEvent]) -> GhostGuardReport {
    let mut findings = Vec::new();
    let mut sensitive_apps = BTreeSet::new();

    if events.is_empty() {
        findings.push(GuardFinding {
            severity: GuardSeverity::Medium,
            category: GuardCategory::EmptyWorkflow,
            step_index: None,
            title: "No workflow steps to audit".into(),
            detail: "Ghost Guard could not inspect an empty workflow.".into(),
            recommendation: "Record a short, low-risk task before saving or replaying.".into(),
        });
    }

    for (idx, event) in events.iter().enumerate() {
        match event {
            InputEvent::MouseClick {
                element,
                x,
                y,
                semantic_tag,
                ..
            } => {
                if let Some(el) = element {
                    let haystack = element_text(el);
                    if is_sensitive_element(el) || contains_any(&haystack, SENSITIVE_APP_HINTS) {
                        if !el.app.is_empty() && el.app != "Unknown" {
                            sensitive_apps.insert(el.app.clone());
                        }
                        findings.push(GuardFinding {
                            severity: GuardSeverity::High,
                            category: if is_sensitive_element(el) { GuardCategory::SensitiveField } else { GuardCategory::SensitiveApp },
                            step_index: Some(idx),
                            title: "Sensitive app or surface detected".into(),
                            detail: format!("Step {} touches {}.", idx + 1, readable_element(el)),
                            recommendation: "Ghost suppresses keystroke capture after clicks into sensitive fields. Re-record and stop before entering secrets; replay should require manual confirmation.".into(),
                        });
                    }
                    if contains_any(&haystack, DESTRUCTIVE_HINTS) {
                        findings.push(GuardFinding {
                            severity: GuardSeverity::High,
                            category: GuardCategory::DestructiveAction,
                            step_index: Some(idx),
                            title: "Potentially irreversible action".into(),
                            detail: format!(
                                "Step {} may activate {}.",
                                idx + 1,
                                readable_element(el)
                            ),
                            recommendation:
                                "Run this workflow step-by-step and pause before this action."
                                    .into(),
                        });
                    }
                } else {
                    findings.push(GuardFinding {
                        severity: GuardSeverity::Low,
                        category: GuardCategory::LowLocatorConfidence,
                        step_index: Some(idx),
                        title: "Coordinate-only click".into(),
                        detail: format!("Step {} clicks absolute coordinates ({}, {}) without accessible metadata.", idx + 1, x, y),
                        recommendation: "Use Inspect Element or re-record with app accessibility enabled so replay can target semantic UI metadata.".into(),
                    });
                }
                if semantic_tag.as_ref().is_some_and(|tag| tag.ai_generated) {
                    findings.push(GuardFinding {
                        severity: GuardSeverity::Medium,
                        category: GuardCategory::ExternalDraft,
                        step_index: Some(idx),
                        title: "AI-generated step".into(),
                        detail: format!("Step {} was generated or modified by AI.", idx + 1),
                        recommendation: "Review the target and dry-run before allowing execution."
                            .into(),
                    });
                }
            }
            InputEvent::Key {
                chars,
                action,
                semantic_tag,
                ..
            } => {
                if matches!(action, KeyAction::Down) && looks_like_secret(chars) {
                    findings.push(GuardFinding {
                        severity: GuardSeverity::Critical,
                        category: GuardCategory::CredentialInput,
                        step_index: Some(idx),
                        title: "Possible credential typed".into(),
                        detail: format!("Step {} types text that looks like a secret or token.", idx + 1),
                        recommendation: "Delete this step and use a manual checkpoint instead of replaying secrets.".into(),
                    });
                }
                if semantic_tag.as_ref().is_some_and(|tag| tag.ai_generated) {
                    findings.push(GuardFinding {
                        severity: GuardSeverity::Medium,
                        category: GuardCategory::ExternalDraft,
                        step_index: Some(idx),
                        title: "AI-generated keyboard step".into(),
                        detail: format!("Step {} was generated or modified by AI.", idx + 1),
                        recommendation: "Review typed text before replay.".into(),
                    });
                }
            }
            _ => {}
        }
    }

    finalize_report(events.len(), sensitive_apps, findings)
}

/// Audit a **compressed semantic timeline** rather than the raw event stream.
///
/// This is the guard's view of the same read model the reviewer sees: every
/// finding's `step_index` refers to a *semantic* [`CompressedStep`] position
/// (aligned with the compression review timeline), not a raw-event index. It is
/// the routing point for recorded routines — compressed steps flow through the
/// same deterministic risk classes (`sensitive`, `destructive`, `credential`,
/// `low-locator-confidence`, `replay-reliability`) that gate raw workflows.
///
/// It is intentionally local and pure: no LLM, no network, no file IO. Typed
/// text is already redacted by the compressor before it reaches here, so this
/// audit never sees retained secure-field text.
pub fn audit_compressed(report: &CompressionReport) -> GhostGuardReport {
    let mut findings = Vec::new();
    let mut sensitive_apps = BTreeSet::new();

    if report.steps.is_empty() {
        findings.push(GuardFinding {
            severity: GuardSeverity::Medium,
            category: GuardCategory::EmptyWorkflow,
            step_index: None,
            title: "No workflow steps to audit".into(),
            detail: "Ghost Guard could not inspect an empty semantic timeline.".into(),
            recommendation: "Record a short, low-risk task before saving or replaying.".into(),
        });
    }

    for (idx, step) in report.steps.iter().enumerate() {
        match step {
            CompressedStep::Click(click) => match &click.target {
                Some(target) => {
                    let haystack = target_text(target);
                    if target_is_sensitive(target) || contains_any(&haystack, SENSITIVE_APP_HINTS) {
                        note_sensitive_app(&mut sensitive_apps, target);
                        findings.push(GuardFinding {
                            severity: GuardSeverity::High,
                            category: if target_is_sensitive(target) {
                                GuardCategory::SensitiveField
                            } else {
                                GuardCategory::SensitiveApp
                            },
                            step_index: Some(idx),
                            title: "Sensitive app or surface detected".into(),
                            detail: format!("Step {} touches {}.", idx + 1, readable_target(target)),
                            recommendation: "Replay should pause here and require manual confirmation before interacting with a sensitive surface.".into(),
                        });
                    }
                    if contains_any(&haystack, DESTRUCTIVE_HINTS) {
                        findings.push(GuardFinding {
                            severity: GuardSeverity::High,
                            category: GuardCategory::DestructiveAction,
                            step_index: Some(idx),
                            title: "Potentially irreversible action".into(),
                            detail: format!(
                                "Step {} may activate {}.",
                                idx + 1,
                                readable_target(target)
                            ),
                            recommendation:
                                "Run this routine step-by-step and pause before this action."
                                    .into(),
                        });
                    }
                }
                None => {
                    findings.push(GuardFinding {
                        severity: GuardSeverity::Low,
                        category: GuardCategory::LowLocatorConfidence,
                        step_index: Some(idx),
                        title: "Coordinate-only click".into(),
                        detail: format!(
                            "Step {} clicks {} without accessible metadata.",
                            idx + 1,
                            click
                                .fallback_coords
                                .map(|(x, y)| format!("absolute coordinates ({x}, {y})"))
                                .unwrap_or_else(|| "an unknown location".into())
                        ),
                        recommendation: "Re-record with app accessibility enabled so replay can target semantic UI metadata instead of screen coordinates.".into(),
                    });
                }
            },
            CompressedStep::TypeText(typed) => {
                if typed.text.as_deref().is_some_and(looks_like_secret) {
                    findings.push(GuardFinding {
                        severity: GuardSeverity::Critical,
                        category: GuardCategory::CredentialInput,
                        step_index: Some(idx),
                        title: "Possible credential typed".into(),
                        detail: format!(
                            "Step {} types text that looks like a secret or token.",
                            idx + 1
                        ),
                        recommendation: "Delete this step and use a manual checkpoint instead of replaying secrets.".into(),
                    });
                } else if typed.secure_field
                    || typed.target.as_ref().is_some_and(target_is_sensitive)
                {
                    if let Some(target) = &typed.target {
                        note_sensitive_app(&mut sensitive_apps, target);
                    }
                    findings.push(GuardFinding {
                        severity: GuardSeverity::High,
                        category: GuardCategory::SensitiveField,
                        step_index: Some(idx),
                        title: "Typing into a sensitive field".into(),
                        detail: format!(
                            "Step {} types {} characters into a secure or credential-like field.",
                            idx + 1,
                            typed.char_count
                        ),
                        recommendation: "Replace this step with a manual checkpoint so Ghost never replays credentials.".into(),
                    });
                }
            }
            CompressedStep::Shortcut(shortcut) => {
                let haystack = format!(
                    "{} {}",
                    shortcut.combo,
                    shortcut.action.clone().unwrap_or_default()
                )
                .to_lowercase();
                if contains_any(&haystack, DESTRUCTIVE_HINTS) {
                    findings.push(GuardFinding {
                        severity: GuardSeverity::Medium,
                        category: GuardCategory::DestructiveAction,
                        step_index: Some(idx),
                        title: "Potentially destructive shortcut".into(),
                        detail: format!("Step {} presses {}.", idx + 1, shortcut.combo),
                        recommendation: "Confirm this shortcut is safe to replay unattended."
                            .into(),
                    });
                }
            }
            CompressedStep::Unknown(unknown) => {
                findings.push(GuardFinding {
                    severity: GuardSeverity::Medium,
                    category: GuardCategory::ReplayReliability,
                    step_index: Some(idx),
                    title: "Unclassified step".into(),
                    detail: format!(
                        "Step {} could not be classified: {}.",
                        idx + 1,
                        unknown.description
                    ),
                    recommendation:
                        "Review this step manually; unclassified steps may not replay reliably."
                            .into(),
                });
            }
            CompressedStep::Scroll(_) | CompressedStep::Wait(_) => {}
        }

        // Fold the compressor's own confidence signal in per semantic step.
        // Unknown steps (confidence 0.0) are covered above, and a coordinate-only
        // click already earns a LowLocatorConfidence finding, so only flag
        // click/type steps that *have* a target resolved with low confidence.
        let confidence = step.confidence();
        let has_target = match step {
            CompressedStep::Click(c) => c.target.is_some(),
            CompressedStep::TypeText(t) => t.target.is_some(),
            _ => false,
        };
        if has_target && confidence > 0.0 && confidence <= LOW_CONFIDENCE {
            findings.push(GuardFinding {
                severity: GuardSeverity::Low,
                category: GuardCategory::ReplayReliability,
                step_index: Some(idx),
                title: "Low-confidence target".into(),
                detail: format!(
                    "Step {} resolved its target with low confidence ({:.0}%).",
                    idx + 1,
                    confidence * 100.0
                ),
                recommendation: "Dry-run this step before an unattended replay.".into(),
            });
        }
    }

    finalize_report(report.steps.len(), sensitive_apps, findings)
}

/// Score, classify, summarize, and assemble a [`GhostGuardReport`] from a set of
/// findings. Shared by [`audit_workflow`] (raw events) and [`audit_compressed`]
/// (semantic steps) so both surfaces score risk identically. `unit_count` is the
/// number of items audited (raw events or compressed steps) and feeds the
/// report's `event_count`.
fn finalize_report(
    unit_count: usize,
    sensitive_apps: BTreeSet<String>,
    findings: Vec<GuardFinding>,
) -> GhostGuardReport {
    let penalty: u16 = findings
        .iter()
        .map(|f| match f.severity {
            GuardSeverity::Low => 5,
            GuardSeverity::Medium => 15,
            GuardSeverity::High => 30,
            GuardSeverity::Critical => 50,
        })
        .sum();
    let score = 100u16
        .saturating_sub(penalty)
        .max(if unit_count == 0 { 0 } else { 10 }) as u8;
    let requires_confirmation = findings
        .iter()
        .any(|f| matches!(f.severity, GuardSeverity::High | GuardSeverity::Critical));
    let blocks_replay = findings
        .iter()
        .any(|f| matches!(f.severity, GuardSeverity::Critical));
    let safe_to_save = !findings
        .iter()
        .any(|f| matches!(f.category, GuardCategory::CredentialInput));
    let risk_level = match score {
        85..=100 => "low",
        65..=84 => "medium",
        35..=64 => "high",
        _ => "critical",
    }
    .to_string();
    let summary = if findings.is_empty() {
        "No obvious privacy or destructive-action risks detected. Still watch the first replay."
            .to_string()
    } else {
        format!(
            "Ghost Guard found {} issue(s). {} require extra confirmation.",
            findings.len(),
            if requires_confirmation {
                "Some"
            } else {
                "None"
            }
        )
    };

    let ai_audit = build_audit_layer(unit_count, &findings, blocks_replay);

    GhostGuardReport {
        score,
        risk_level,
        event_count: unit_count,
        sensitive_apps: sensitive_apps.into_iter().collect(),
        findings,
        requires_confirmation,
        blocks_replay,
        safe_to_save,
        summary,
        ai_audit,
    }
}

fn element_text(el: &ElementInfo) -> String {
    format!(
        "{} {} {} {} {} {}",
        el.app,
        el.role,
        el.name,
        el.value.clone().unwrap_or_default(),
        el.description.clone().unwrap_or_default(),
        el.identifier.clone().unwrap_or_default()
    )
    .to_lowercase()
}

fn readable_element(el: &ElementInfo) -> String {
    let role = el.role_description.as_deref().unwrap_or(&el.role);
    let name = if el.name.is_empty() {
        "unnamed"
    } else {
        &el.name
    };
    let app = if el.app.is_empty() {
        "Unknown"
    } else {
        &el.app
    };
    format!("{} \"{}\" in {}", role, name, app)
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn target_text(target: &Target) -> String {
    format!(
        "{} {} {} {}",
        target.app,
        target.role,
        target.name,
        target.identifier.clone().unwrap_or_default()
    )
    .to_lowercase()
}

fn readable_target(target: &Target) -> String {
    let name = if target.name.is_empty() {
        "unnamed"
    } else {
        &target.name
    };
    let app = if target.app.is_empty() {
        "Unknown"
    } else {
        &target.app
    };
    format!("{} \"{}\" in {}", target.role, name, app)
}

fn note_sensitive_app(apps: &mut BTreeSet<String>, target: &Target) {
    if !target.app.is_empty() && target.app != "Unknown" {
        apps.insert(target.app.clone());
    }
}

/// Whether a compressed-step [`Target`] looks like a credential/secure surface.
/// Mirrors [`is_sensitive_element`] but works on the reduced compressed target
/// (which carries no `value`/`description`/`role_description`).
pub fn target_is_sensitive(target: &Target) -> bool {
    let role = target.role.to_ascii_lowercase();
    role.contains("securetextfield")
        || role.contains("password")
        || contains_any(&target_text(target), CREDENTIAL_HINTS)
}

pub fn is_sensitive_element(el: &ElementInfo) -> bool {
    let role = el.role.to_ascii_lowercase();
    let role_description = el
        .role_description
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let haystack = element_text(el);

    role.contains("securetextfield")
        || role.contains("password")
        || role_description.contains("secure")
        || role_description.contains("password")
        || contains_any(&haystack, CREDENTIAL_HINTS)
}

pub fn should_suppress_keyboard_after_click(event: &InputEvent) -> bool {
    matches!(
        event,
        InputEvent::MouseClick {
            element: Some(el),
            button: 0 | 2,
            ..
        } if is_sensitive_element(el) || contains_any(&element_text(el), SENSITIVE_APP_HINTS)
    )
}

pub fn sanitize_recorded_event(event: InputEvent, suppress_keyboard: bool) -> Option<InputEvent> {
    match event {
        InputEvent::Key {
            action: KeyAction::Down,
            ..
        } if suppress_keyboard => None,
        InputEvent::Key { chars, .. } if looks_like_secret(&chars) => None,
        other => Some(other),
    }
}

fn build_audit_layer(
    unit_count: usize,
    findings: &[GuardFinding],
    blocks_replay: bool,
) -> GuardAuditLayer {
    let credential_count = findings
        .iter()
        .filter(|f| {
            matches!(
                f.category,
                GuardCategory::CredentialInput | GuardCategory::SensitiveField
            )
        })
        .count();
    let coordinate_clicks = findings
        .iter()
        .filter(|f| matches!(f.category, GuardCategory::LowLocatorConfidence))
        .count();
    let blocked_steps = findings
        .iter()
        .filter(|f| matches!(f.severity, GuardSeverity::Critical))
        .filter_map(|f| f.step_index.map(|idx| idx + 1))
        .collect::<Vec<_>>();

    let mut recommended_next_steps = Vec::new();
    if credential_count > 0 {
        recommended_next_steps.push("Re-record the workflow and stop before entering passwords, OTPs, tokens, payment data, or other secrets.".to_string());
    }
    if coordinate_clicks > 0 {
        recommended_next_steps.push("Re-record with accessibility metadata available or inspect coordinate-only steps before replay.".to_string());
    }
    if blocks_replay {
        recommended_next_steps.push(
            "Remove blocked steps before replay; use a manual checkpoint for sensitive input."
                .to_string(),
        );
    }
    if recommended_next_steps.is_empty() {
        recommended_next_steps.push("Run the first replay while watching, then save only if the result matches your intent.".to_string());
    }

    GuardAuditLayer {
        privacy_summary: if credential_count > 0 {
            format!("Detected {credential_count} sensitive-input risk(s). Ghost should not store or replay those values.")
        } else {
            "No obvious stored-password, token, or payment-field input was detected.".to_string()
        },
        replay_summary: if coordinate_clicks > 0 {
            format!("{coordinate_clicks} click step(s) rely only on screen coordinates, which can break when windows move.")
        } else if unit_count == 0 {
            "There are no steps to replay yet.".to_string()
        } else {
            "Replay has usable semantic metadata or non-click steps for this local audit."
                .to_string()
        },
        recommended_next_steps,
        blocked_steps,
    }
}

fn looks_like_secret(chars: &str) -> bool {
    let lower = chars.to_lowercase();
    contains_any(&lower, CREDENTIAL_HINTS)
        || (chars.len() >= 20
            && chars.chars().any(|c| c.is_ascii_digit())
            && chars.chars().any(|c| c.is_ascii_uppercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(role: &str, name: &str, app: &str) -> ElementInfo {
        ElementInfo {
            role: role.to_string(),
            name: name.to_string(),
            app: app.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn sensitive_element_detection_catches_password_fields() {
        let el = element("AXSecureTextField", "Password", "Demo Login");
        assert!(is_sensitive_element(&el));
    }

    #[test]
    fn sanitizer_drops_keyboard_when_sensitive_context_active() {
        let event = InputEvent::Key {
            code: 1,
            chars: "a".to_string(),
            modifiers: 0,
            action: KeyAction::Down,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
        };
        assert!(sanitize_recorded_event(event, true).is_none());
    }

    #[test]
    fn audit_blocks_secret_like_text() {
        let report = audit_workflow(&[InputEvent::Key {
            code: 1,
            chars: "sk-SECRETTokenValue123456789".to_string(),
            modifiers: 0,
            action: KeyAction::Down,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
        }]);
        assert!(report.blocks_replay);
        assert!(!report.safe_to_save);
        assert_eq!(report.ai_audit.blocked_steps, vec![1]);
    }

    // --- helpers ---------------------------------------------------------

    fn click(element: Option<ElementInfo>, button: u8) -> InputEvent {
        InputEvent::MouseClick {
            x: 10,
            y: 20,
            button,
            element,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
            self_heal: None,
        }
    }

    fn key_down(chars: &str) -> InputEvent {
        InputEvent::Key {
            code: 1,
            chars: chars.to_string(),
            modifiers: 0,
            action: KeyAction::Down,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
        }
    }

    // --- looks_like_secret ----------------------------------------------

    #[test]
    fn looks_like_secret_matches_credential_keywords() {
        assert!(looks_like_secret("my password is here"));
        assert!(looks_like_secret("OTP"));
        assert!(looks_like_secret("enter your api key"));
        assert!(looks_like_secret("cvv"));
    }

    #[test]
    fn looks_like_secret_matches_high_entropy_tokens() {
        // >= 20 chars with at least one digit and one uppercase letter.
        assert!(looks_like_secret("ABCDEFGHIJ1234567890"));
    }

    #[test]
    fn looks_like_secret_ignores_benign_text() {
        assert!(!looks_like_secret("hello"));
        assert!(!looks_like_secret("click the submit button"));
        // Long, but all-lowercase with no credential keyword: not secret-shaped.
        assert!(!looks_like_secret("abcdefghij1234567890"));
    }

    // --- is_sensitive_element -------------------------------------------

    #[test]
    fn sensitive_element_detection_uses_credential_hints_in_name() {
        assert!(is_sensitive_element(&element(
            "AXTextField",
            "Card number",
            "Shop"
        )));
    }

    #[test]
    fn sensitive_element_detection_uses_secure_role_description() {
        let el = ElementInfo {
            role: "AXTextField".to_string(),
            name: "Field".to_string(),
            app: "App".to_string(),
            role_description: Some("Secure entry".to_string()),
            ..Default::default()
        };
        assert!(is_sensitive_element(&el));
    }

    #[test]
    fn sensitive_element_detection_ignores_benign_fields() {
        assert!(!is_sensitive_element(&element("AXButton", "Save", "Notes")));
    }

    // --- should_suppress_keyboard_after_click ---------------------------

    #[test]
    fn suppress_after_sensitive_left_or_right_click_only() {
        let sensitive = || Some(element("AXSecureTextField", "Password", "Login"));
        assert!(should_suppress_keyboard_after_click(&click(sensitive(), 0))); // left
        assert!(should_suppress_keyboard_after_click(&click(sensitive(), 2))); // right
                                                                               // Middle-click (button 1) is not a field-focusing interaction.
        assert!(!should_suppress_keyboard_after_click(&click(
            sensitive(),
            1
        )));
    }

    #[test]
    fn suppress_does_not_trigger_for_benign_or_coordinate_clicks() {
        let benign = Some(element("AXButton", "Save", "Notes"));
        assert!(!should_suppress_keyboard_after_click(&click(benign, 0)));
        assert!(!should_suppress_keyboard_after_click(&click(None, 0)));
    }

    #[test]
    fn suppress_triggers_for_microsoft_sensitive_apps() {
        let teams = Some(element("AXButton", "Send message", "Teams"));
        let outlook = Some(element("AXTextField", "Subject", "Outlook"));
        let copilot = Some(element("AXButton", "Ask Copilot", "Copilot"));
        
        assert!(should_suppress_keyboard_after_click(&click(teams, 0)));
        assert!(should_suppress_keyboard_after_click(&click(outlook, 0)));
        assert!(should_suppress_keyboard_after_click(&click(copilot, 0)));
    }

    // --- sanitize_recorded_event ----------------------------------------

    #[test]
    fn sanitizer_drops_secret_text_even_without_active_suppression() {
        assert!(sanitize_recorded_event(key_down("my password"), false).is_none());
    }

    #[test]
    fn sanitizer_passes_benign_keys_and_mouse_events() {
        assert!(sanitize_recorded_event(key_down("a"), false).is_some());
        // Mouse events are never suppressed, even while keyboard capture is paused.
        assert!(sanitize_recorded_event(click(None, 0), true).is_some());
    }

    // --- audit_workflow --------------------------------------------------

    #[test]
    fn audit_flags_empty_workflow_without_requiring_confirmation() {
        let report = audit_workflow(&[]);
        assert_eq!(report.event_count, 0);
        assert!(report
            .findings
            .iter()
            .any(|f| f.category == GuardCategory::EmptyWorkflow));
        assert!(!report.requires_confirmation);
    }

    #[test]
    fn audit_flags_destructive_action_and_requires_confirmation() {
        let report = audit_workflow(&[click(Some(element("AXButton", "Delete", "Files")), 0)]);
        assert!(report
            .findings
            .iter()
            .any(|f| f.category == GuardCategory::DestructiveAction));
        assert!(report.requires_confirmation);
        // High severity, but not a credential, so the workflow is still saveable.
        assert!(!report.blocks_replay);
        assert!(report.safe_to_save);
    }

    #[test]
    fn audit_flags_coordinate_only_clicks_as_low_confidence() {
        let report = audit_workflow(&[click(None, 0)]);
        let finding = report
            .findings
            .iter()
            .find(|f| f.category == GuardCategory::LowLocatorConfidence)
            .expect("expected a low-confidence finding");
        assert_eq!(finding.severity, GuardSeverity::Low);
        assert!(!report.requires_confirmation);
    }

    #[test]
    fn audit_records_sensitive_app_in_report() {
        let report = audit_workflow(&[click(
            Some(element("AXButton", "Accounts", "Bank of America")),
            0,
        )]);
        assert!(report
            .findings
            .iter()
            .any(|f| f.category == GuardCategory::SensitiveApp));
        assert!(report
            .sensitive_apps
            .contains(&"Bank of America".to_string()));
    }

    #[test]
    fn audit_clean_workflow_is_low_risk_and_saveable() {
        let report = audit_workflow(&[click(Some(element("AXButton", "Save", "Notes")), 0)]);
        assert!(report.findings.is_empty());
        assert_eq!(report.score, 100);
        assert_eq!(report.risk_level, "low");
        assert!(report.safe_to_save);
        assert!(!report.requires_confirmation);
    }

    // --- audit_compressed (semantic-timeline routing) --------------------

    use crate::core::compression::{
        ClickButton, ClickStep, CompressedStep, CompressionReport, ShortcutStep, Target,
        TypeTextStep, UnknownStep,
    };

    fn report_of(steps: Vec<CompressedStep>) -> CompressionReport {
        let spans = steps.iter().enumerate().map(|(i, _)| (i, 1)).collect();
        CompressionReport::new(steps.len(), steps, spans)
    }

    fn target(name: &str, role: &str, app: &str) -> Target {
        Target {
            name: name.to_string(),
            role: role.to_string(),
            app: app.to_string(),
            identifier: None,
        }
    }

    fn click_step(target: Option<Target>, confidence: f32) -> CompressedStep {
        CompressedStep::Click(ClickStep {
            button: ClickButton::Left,
            target,
            fallback_coords: Some((10, 20)),
            confidence,
            raw_event_count: 1,
        })
    }

    fn type_step(target: Option<Target>, secure_field: bool, text: Option<&str>) -> CompressedStep {
        CompressedStep::TypeText(TypeTextStep {
            char_count: text.map(str::len).unwrap_or(8),
            redacted: text.is_none(),
            secure_field,
            target,
            text: text.map(str::to_string),
            confidence: 0.9,
            raw_event_count: 8,
        })
    }

    #[test]
    fn target_is_sensitive_detects_credential_surfaces() {
        assert!(target_is_sensitive(&target(
            "Password",
            "AXSecureTextField",
            "Login"
        )));
        assert!(target_is_sensitive(&target(
            "Card number",
            "AXTextField",
            "Shop"
        )));
        assert!(!target_is_sensitive(&target("Save", "AXButton", "Notes")));
    }

    #[test]
    fn compressed_audit_flags_empty_timeline_without_confirmation() {
        let report = audit_compressed(&report_of(vec![]));
        assert_eq!(report.event_count, 0);
        assert!(report
            .findings
            .iter()
            .any(|f| f.category == GuardCategory::EmptyWorkflow));
        assert!(!report.requires_confirmation);
    }

    #[test]
    fn compressed_audit_clean_timeline_is_low_risk() {
        let report = audit_compressed(&report_of(vec![click_step(
            Some(target("Save", "AXButton", "Notes")),
            0.95,
        )]));
        assert!(report.findings.is_empty());
        assert_eq!(report.score, 100);
        assert_eq!(report.risk_level, "low");
        assert!(report.safe_to_save);
        assert!(!report.requires_confirmation);
    }

    #[test]
    fn compressed_audit_flags_secure_field_typing_as_high() {
        let report = audit_compressed(&report_of(vec![type_step(
            Some(target("Password", "AXSecureTextField", "Login")),
            true,
            None,
        )]));
        let finding = report
            .findings
            .iter()
            .find(|f| f.category == GuardCategory::SensitiveField)
            .expect("expected a sensitive-field finding");
        assert_eq!(finding.severity, GuardSeverity::High);
        assert!(report.requires_confirmation);
        // Secure-field text is already redacted by the compressor, so the
        // timeline holds no secret — it stays saveable but not auto-replayable.
        assert!(!report.blocks_replay);
        assert!(report.safe_to_save);
    }

    #[test]
    fn compressed_audit_blocks_secret_like_typed_text() {
        let report = audit_compressed(&report_of(vec![type_step(
            Some(target("API key", "AXTextField", "Console")),
            false,
            Some("sk-SECRETTokenValue123456789"),
        )]));
        assert!(report
            .findings
            .iter()
            .any(|f| f.category == GuardCategory::CredentialInput));
        assert!(report.blocks_replay);
        assert!(!report.safe_to_save);
    }

    #[test]
    fn compressed_audit_flags_destructive_click_and_records_sensitive_app() {
        let report = audit_compressed(&report_of(vec![click_step(
            Some(target("Delete account", "AXButton", "Bank")),
            0.95,
        )]));
        assert!(report
            .findings
            .iter()
            .any(|f| f.category == GuardCategory::DestructiveAction));
        // "bank" also trips the sensitive-app hint list.
        assert!(report.sensitive_apps.contains(&"Bank".to_string()));
        assert!(report.requires_confirmation);
    }

    #[test]
    fn compressed_audit_flags_coordinate_only_click_as_low_confidence() {
        let report = audit_compressed(&report_of(vec![click_step(None, 0.25)]));
        let finding = report
            .findings
            .iter()
            .find(|f| f.category == GuardCategory::LowLocatorConfidence)
            .expect("expected a low-confidence locator finding");
        assert_eq!(finding.severity, GuardSeverity::Low);
        // A coordinate-only click is flagged exactly once (no duplicate
        // reliability finding from the confidence fold).
        assert_eq!(report.findings.len(), 1);
        assert!(!report.requires_confirmation);
    }

    #[test]
    fn compressed_audit_flags_low_confidence_target_for_reliability() {
        let report = audit_compressed(&report_of(vec![click_step(
            Some(target("Button", "AXButton", "App")),
            0.4,
        )]));
        assert!(report
            .findings
            .iter()
            .any(|f| f.category == GuardCategory::ReplayReliability));
    }

    #[test]
    fn compressed_audit_flags_unknown_step_as_reliability_risk() {
        let report = audit_compressed(&report_of(vec![CompressedStep::Unknown(UnknownStep {
            description: "Press Escape".to_string(),
            raw_event_count: 1,
        })]));
        let finding = report
            .findings
            .iter()
            .find(|f| f.category == GuardCategory::ReplayReliability)
            .expect("expected an unclassified-step reliability finding");
        assert_eq!(finding.severity, GuardSeverity::Medium);
    }

    #[test]
    fn compressed_audit_flags_destructive_shortcut() {
        let report = audit_compressed(&report_of(vec![CompressedStep::Shortcut(ShortcutStep {
            combo: "cmd+backspace".to_string(),
            action: Some("delete".to_string()),
            raw_event_count: 1,
        })]));
        assert!(report
            .findings
            .iter()
            .any(|f| f.category == GuardCategory::DestructiveAction));
    }

    #[test]
    fn compressed_audit_finding_step_index_is_semantic_not_raw() {
        // A benign step, then a destructive click at semantic position 1,
        // even though the raw stream that produced it may have many events.
        let report = audit_compressed(&report_of(vec![
            click_step(Some(target("Home", "AXButton", "App")), 0.95),
            click_step(Some(target("Delete", "AXButton", "App")), 0.95),
        ]));
        let finding = report
            .findings
            .iter()
            .find(|f| f.category == GuardCategory::DestructiveAction)
            .expect("expected a destructive finding");
        assert_eq!(finding.step_index, Some(1));
    }
}
