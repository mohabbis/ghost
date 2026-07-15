//! Conflict detection for proposed moves.
//!
//! A conflict is anything that would make a naive move clobber data or collide
//! with another planned move. The planner resolves conflicts by choosing a
//! de-duplicated name (see [`super::naming::deduplicate`]) and **records the
//! conflict on the action** so the user sees it — the Organizer never silently
//! overwrites.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Why a proposed target collided, before de-duplication resolved it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Conflict {
    /// A file already exists on disk at the originally proposed target.
    TargetExists { original_target: String },
    /// Another file in this same plan was already assigned that target.
    DuplicateInPlan { original_target: String },
}

impl Conflict {
    /// A short human-readable explanation for the preview UI.
    pub fn describe(&self) -> String {
        match self {
            Conflict::TargetExists { original_target } => {
                format!(
                    "A file already exists at {original_target}; will be renamed to avoid overwrite"
                )
            }
            Conflict::DuplicateInPlan { original_target } => {
                format!(
                    "Another planned file targets {original_target}; will be renamed to keep both"
                )
            }
        }
    }
}

/// Does a file already exist on disk at `target`? Read-only `stat`; does not
/// follow up with any mutation. A target whose path is a directory also counts
/// as existing (we would not overwrite it).
pub fn exists_on_disk(target: &Path) -> bool {
    target.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::testutil::tempdir;

    #[test]
    fn exists_on_disk_detects_present_file() {
        let tmp = tempdir();
        let f = tmp.file("here.txt", b"x");
        assert!(exists_on_disk(&f));
        assert!(!exists_on_disk(&tmp.path().join("absent.txt")));
    }

    #[test]
    fn describe_mentions_no_overwrite() {
        let c = Conflict::TargetExists {
            original_target: "/x/y.pdf".into(),
        };
        assert!(c.describe().to_lowercase().contains("overwrite"));
    }
}
