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
