//! Ghost Guard: local workflow risk audit before replay.
//!
//! The guard is intentionally deterministic and local-first. It does not try to
//! be a cloud AI classifier; it gives users an explainable safety review of the
//! workflow they just recorded.

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
    DestructiveAction,
    CredentialInput,
    LowLocatorConfidence,
    ExternalDraft,
    EmptyWorkflow,
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
    pub summary: String,
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
    "terminal",
    "system settings",
    "settings",
];

const DESTRUCTIVE_HINTS: &[&str] = &[
    "delete", "remove", "trash", "send", "submit", "purchase", "pay", "transfer", "install", "run",
    "execute", "confirm",
];

const CREDENTIAL_HINTS: &[&str] = &["password", "passcode", "token", "secret", "api key", "otp"];

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
                    if contains_any(&haystack, SENSITIVE_APP_HINTS) {
                        if !el.app.is_empty() && el.app != "Unknown" {
                            sensitive_apps.insert(el.app.clone());
                        }
                        findings.push(GuardFinding {
                            severity: GuardSeverity::High,
                            category: GuardCategory::SensitiveApp,
                            step_index: Some(idx),
                            title: "Sensitive app or surface detected".into(),
                            detail: format!("Step {} touches {}.", idx + 1, readable_element(el)),
                            recommendation: "Require manual confirmation before replaying this step, or add the app to a blocklist.".into(),
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
        .max(if events.is_empty() { 0 } else { 10 }) as u8;
    let requires_confirmation = findings
        .iter()
        .any(|f| matches!(f.severity, GuardSeverity::High | GuardSeverity::Critical));
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

    GhostGuardReport {
        score,
        risk_level,
        event_count: events.len(),
        sensitive_apps: sensitive_apps.into_iter().collect(),
        findings,
        requires_confirmation,
        summary,
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

fn looks_like_secret(chars: &str) -> bool {
    let lower = chars.to_lowercase();
    contains_any(&lower, CREDENTIAL_HINTS)
        || (chars.len() >= 20
            && chars.chars().any(|c| c.is_ascii_digit())
            && chars.chars().any(|c| c.is_ascii_uppercase()))
}
