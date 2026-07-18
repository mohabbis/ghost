//! Shared semantic locator format for Routines / Action Plan UI steps.
//!
//! This module is the Rust half of the locator contract described in
//! `docs/macos-automation-architecture.md`. Capture (Swift) and execution (Rust)
//! should converge on these shapes. Locators are identity/data only — policy and
//! approval stay in the Rust trust core.

use serde::{Deserialize, Serialize};

/// Constraint on one ancestor (or self) in an accessibility path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AxConstraint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
}

/// Normalized point in window or display space (0.0–1.0 when known).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedPoint {
    pub x: f64,
    pub y: f64,
}

/// Optional axis-aligned region in the same space as [`NormalizedPoint`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Multi-strategy locator. Prefer [`Locator::Accessibility`] when the AX tree
/// is sufficient; fall back to text/OCR, image template, then coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "strategy")]
pub enum Locator {
    Accessibility {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        identifier: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        ancestor_path: Vec<AxConstraint>,
    },
    Text {
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<NormRect>,
    },
    ImageTemplate {
        template_id: String,
        confidence: f32,
    },
    Coordinates {
        point: NormalizedPoint,
    },
}

/// Pure AX quality score for deciding whether vision fallback is warranted.
///
/// Inspect the tree first; score it; only then switch to vision. Do not treat
/// "Electron" or "Flutter" as automatic vision targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AxQuality {
    /// 0–100 composite score.
    pub score: u8,
    pub actionable: bool,
    pub unique: bool,
    pub has_identifier: bool,
    pub has_role: bool,
    pub has_title: bool,
}

impl AxQuality {
    /// Minimum score (and uniqueness + actionability) for trusting an AX action.
    pub const ACTION_THRESHOLD: u8 = 40;

    pub fn sufficient_for_action(self) -> bool {
        self.unique && self.actionable && self.score >= Self::ACTION_THRESHOLD
    }

    /// When true, Ghost may attempt OCR / visual matching if those strategies
    /// are allowed for the step — not because of the host framework name.
    pub fn prefer_vision_fallback(self) -> bool {
        !self.unique
            || (!self.has_identifier && !self.has_title)
            || self.score < Self::ACTION_THRESHOLD
    }
}

/// Score one AX candidate from attributes Ghost can observe without mutating.
pub fn score_ax_candidate(
    role: Option<&str>,
    title: Option<&str>,
    identifier: Option<&str>,
    actionable: bool,
    match_count: usize,
) -> AxQuality {
    let has_role = role.is_some_and(|r| !r.is_empty() && r != "?");
    let has_title = title.is_some_and(|t| !t.is_empty());
    let has_identifier = identifier.is_some_and(|i| !i.is_empty());
    let unique = match_count == 1;

    let mut score = 0u8;
    if has_role {
        score = score.saturating_add(25);
    }
    if has_title {
        score = score.saturating_add(20);
    }
    if has_identifier {
        score = score.saturating_add(35);
    }
    if actionable {
        score = score.saturating_add(15);
    }
    if unique {
        score = score.saturating_add(5);
    }

    AxQuality {
        score,
        actionable,
        unique,
        has_identifier,
        has_role,
        has_title,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rich_unique_actionable_target_is_sufficient() {
        let q = score_ax_candidate(Some("AXButton"), Some("Save"), Some("save-button"), true, 1);
        assert_eq!(q.score, 100);
        assert!(q.sufficient_for_action());
        assert!(!q.prefer_vision_fallback());
    }

    #[test]
    fn role_only_ambiguous_prefers_vision() {
        let q = score_ax_candidate(Some("AXButton"), None, None, true, 4);
        assert!(q.prefer_vision_fallback());
        assert!(!q.sufficient_for_action());
    }

    #[test]
    fn empty_tree_signals_insufficient_ax() {
        let q = score_ax_candidate(None, None, None, false, 0);
        assert_eq!(q.score, 0);
        assert!(q.prefer_vision_fallback());
    }

    #[test]
    fn locator_accessibility_roundtrips_json() {
        let loc = Locator::Accessibility {
            role: Some("AXTextArea".into()),
            title: None,
            identifier: Some("doc".into()),
            value: None,
            ancestor_path: vec![AxConstraint {
                role: Some("AXWindow".into()),
                title: Some("Untitled".into()),
                identifier: None,
            }],
        };
        let json = serde_json::to_string(&loc).unwrap();
        let back: Locator = serde_json::from_str(&json).unwrap();
        assert_eq!(loc, back);
        assert!(json.contains("\"strategy\":\"accessibility\""));
    }
}
