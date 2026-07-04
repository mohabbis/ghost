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
    #[serde(default)]
    pub rename_dated: bool,
}

/// How far the user trusts mutating operations granted by a folder rule.
///
/// This is the user-facing trust vocabulary: `Automate` runs granted mutations
/// without a per-action confirmation, `AskFirst` requires confirmation (the
/// historical behavior and the serde default, so rules stored before this field
/// existed keep their exact semantics), and `Never` refuses the rule's mutating
/// grants outright while leaving reads usable for planning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Automate,
    #[default]
    AskFirst,
    Never,
}

impl TrustLevel {
    /// The lowercase token stored in SQLite
    /// (`CHECK (... IN ('automate','ask_first','never'))`).
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustLevel::Automate => "automate",
            TrustLevel::AskFirst => "ask_first",
            TrustLevel::Never => "never",
        }
    }

    /// Parse the stored token back into a `TrustLevel`.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "automate" => Some(TrustLevel::Automate),
            "ask_first" => Some(TrustLevel::AskFirst),
            "never" => Some(TrustLevel::Never),
            _ => None,
        }
    }

    /// The more restrictive of two trust levels. Two-sided operations
    /// (move/rename/copy) span two rules; the stricter one governs.
    pub fn stricter(self, other: TrustLevel) -> TrustLevel {
        fn rank(level: TrustLevel) -> u8 {
            match level {
                TrustLevel::Automate => 0,
                TrustLevel::AskFirst => 1,
                TrustLevel::Never => 2,
            }
        }
        if rank(other) > rank(self) {
            other
        } else {
            self
        }
    }
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
    /// How much autonomy the user granted for mutations under this rule.
    /// Defaults to `AskFirst` so rules persisted before this field existed
    /// keep the behavior they were created with.
    #[serde(default)]
    pub trust: TrustLevel,
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
            trust: TrustLevel::AskFirst,
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
            rename_dated: true,
        };
        let json = serde_json::to_string(&zone).unwrap();
        let back: Zone = serde_json::from_str(&json).unwrap();
        assert_eq!(zone, back);
    }

    /// `as_str` must round-trip through `from_token`, and the tokens must match
    /// the SQLite CHECK constraint (`'automate','ask_first','never'`).
    #[test]
    fn trust_level_token_round_trip() {
        for trust in [
            TrustLevel::Automate,
            TrustLevel::AskFirst,
            TrustLevel::Never,
        ] {
            assert_eq!(TrustLevel::from_token(trust.as_str()), Some(trust));
        }
        assert_eq!(TrustLevel::Automate.as_str(), "automate");
        assert_eq!(TrustLevel::AskFirst.as_str(), "ask_first");
        assert_eq!(TrustLevel::Never.as_str(), "never");
        assert_eq!(TrustLevel::from_token("always"), None);
    }

    /// Rules stored (or sent over IPC) before the trust field existed must keep
    /// today's semantics: ask before mutating.
    #[test]
    fn folder_rule_deserializes_missing_trust_as_ask_first() {
        let json = r#"{
            "path": "/home/u/Downloads",
            "can_read": true,
            "can_create": false,
            "can_rename": false,
            "can_move": true,
            "can_copy": false,
            "can_delete": false
        }"#;
        let rule: FolderRule = serde_json::from_str(json).unwrap();
        assert_eq!(rule.trust, TrustLevel::AskFirst);
    }

    /// The wire form is snake_case; pin it so the frontend contract can't drift.
    #[test]
    fn trust_level_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&TrustLevel::AskFirst).unwrap(),
            "\"ask_first\""
        );
        let back: TrustLevel = serde_json::from_str("\"automate\"").unwrap();
        assert_eq!(back, TrustLevel::Automate);
    }

    /// Two-sided operations use the stricter of the two rules' trust levels.
    #[test]
    fn stricter_picks_the_more_restrictive_level() {
        use TrustLevel::*;
        assert_eq!(Automate.stricter(AskFirst), AskFirst);
        assert_eq!(AskFirst.stricter(Automate), AskFirst);
        assert_eq!(AskFirst.stricter(Never), Never);
        assert_eq!(Never.stricter(Automate), Never);
        assert_eq!(Automate.stricter(Automate), Automate);
    }

    #[test]
    fn zone_deserializes_missing_rename_dated_as_false() {
        let json = r#"{
            "id": "z-1",
            "name": "School",
            "description": null,
            "default_decision": "ask"
        }"#;
        let zone: Zone = serde_json::from_str(json).unwrap();
        assert!(!zone.rename_dated);
    }
}
