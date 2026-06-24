//! Zones: user-approved boundaries Ghost is allowed to work inside.
//!
//! These are pure domain types. Persistence lives in `crate::storage`, which
//! depends on this module (never the reverse) to avoid a dependency cycle.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The fallback decision for a Zone when no folder rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DefaultDecision {
    Deny,
    Ask,
    Allow,
}

impl DefaultDecision {
    /// The lowercase token stored in SQLite (`CHECK (... IN ('deny','ask','allow'))`).
    pub fn as_str(&self) -> &'static str {
        match self {
            DefaultDecision::Deny => "deny",
            DefaultDecision::Ask => "ask",
            DefaultDecision::Allow => "allow",
        }
    }

    /// Parse the stored token back into a `DefaultDecision`.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "deny" => Some(DefaultDecision::Deny),
            "ask" => Some(DefaultDecision::Ask),
            "allow" => Some(DefaultDecision::Allow),
            _ => None,
        }
    }
}

/// A user-approved boundary. For the Organizer MVP a Zone is a named set of
/// folder rules with a default decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_decision: DefaultDecision,
}

/// A per-folder permission grant inside a Zone. Operations are only allowed
/// against paths contained within a rule that grants the matching permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderRule {
    pub path: PathBuf,
    pub can_read: bool,
    pub can_create: bool,
    pub can_rename: bool,
    pub can_move: bool,
    pub can_copy: bool,
    pub can_delete: bool,
}

impl FolderRule {
    /// A read-only rule for `path` — the safe default for a newly approved folder.
    pub fn read_only(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            can_read: true,
            can_create: false,
            can_rename: false,
            can_move: false,
            can_copy: false,
            can_delete: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `as_str` must round-trip through `from_token`, and the tokens must match
    /// the values the SQLite migration's CHECK constraint accepts
    /// (`'deny','ask','allow'`). A drift here corrupts persisted Zones.
    #[test]
    fn default_decision_token_round_trip() {
        for decision in [
            DefaultDecision::Deny,
            DefaultDecision::Ask,
            DefaultDecision::Allow,
        ] {
            assert_eq!(
                DefaultDecision::from_token(decision.as_str()),
                Some(decision)
            );
        }
        assert_eq!(DefaultDecision::Deny.as_str(), "deny");
        assert_eq!(DefaultDecision::Ask.as_str(), "ask");
        assert_eq!(DefaultDecision::Allow.as_str(), "allow");
    }

    #[test]
    fn from_token_rejects_unknown_tokens() {
        assert_eq!(DefaultDecision::from_token("DENY"), None);
        assert_eq!(DefaultDecision::from_token("permit"), None);
        assert_eq!(DefaultDecision::from_token(""), None);
    }

    /// The serde `lowercase` representation backs IPC; keep it aligned with the
    /// stored token so the wire form and the DB form never diverge.
    #[test]
    fn default_decision_serializes_as_lowercase() {
        assert_eq!(
            serde_json::to_string(&DefaultDecision::Ask).unwrap(),
            "\"ask\""
        );
        let back: DefaultDecision = serde_json::from_str("\"allow\"").unwrap();
        assert_eq!(back, DefaultDecision::Allow);
    }

    #[test]
    fn read_only_grants_read_and_nothing_else() {
        let rule = FolderRule::read_only("/home/u/Downloads");
        assert_eq!(rule.path, PathBuf::from("/home/u/Downloads"));
        assert!(rule.can_read);
        assert!(!rule.can_create);
        assert!(!rule.can_rename);
        assert!(!rule.can_move);
        assert!(!rule.can_copy);
        assert!(!rule.can_delete);
    }

    #[test]
    fn zone_round_trips_through_json() {
        let zone = Zone {
            id: "z-1".into(),
            name: "School".into(),
            description: Some("coursework".into()),
            default_decision: DefaultDecision::Ask,
        };
        let json = serde_json::to_string(&zone).unwrap();
        let back: Zone = serde_json::from_str(&json).unwrap();
        assert_eq!(zone, back);
    }
}
