//! Execution-time path boundary checks using canonical paths.

use super::engine::evaluate_with_attribution;
use super::{Capability, FolderRule, PolicyDecision};
use std::path::{Path, PathBuf};

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Re-verify that move/rename endpoints still lie inside approved folder rules
/// after canonicalization — catches symlink/TOCTOU path escapes.
pub fn verify_relocate_at_execution(
    from: &Path,
    to: &Path,
    rules: &[FolderRule],
    is_move: bool,
) -> Result<(), String> {
    let from = canonicalize_best_effort(from);
    let to = canonicalize_best_effort(to);
    let cap = if is_move {
        Capability::MoveFile {
            from: from.clone(),
            to: to.clone(),
        }
    } else {
        Capability::RenameFile {
            from: from.clone(),
            to: to.clone(),
        }
    };
    let evaluation = evaluate_with_attribution(&cap, rules);
    if evaluation.decision.is_denied() {
        Err(match evaluation.decision {
            PolicyDecision::Deny { reason } => reason,
            other => format!("relocate denied after canonicalize: {other:?}"),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::testutil::tempdir;
    use crate::policy::TrustLevel;

    fn full_rule(path: &Path) -> FolderRule {
        FolderRule {
            path: path.to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
            trust: TrustLevel::AskFirst,
        }
    }

    #[test]
    fn canonical_paths_inside_zone_pass() {
        let tmp = tempdir();
        let file = tmp.file("doc.pdf", b"x");
        let dest = tmp.dir("dest");
        let rules = vec![full_rule(tmp.path())];
        assert!(verify_relocate_at_execution(&file, &dest.join("doc.pdf"), &rules, true).is_ok());
    }

    #[test]
    fn path_outside_zone_fails_after_canonicalize() {
        let tmp = tempdir();
        let outside = std::env::temp_dir().join(format!("ghost-outside-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&outside).unwrap();
        let file = outside.join("secret.pdf");
        std::fs::write(&file, b"x").unwrap();
        let rules = vec![full_rule(tmp.path())];
        let dest = tmp.path().join("dest/secret.pdf");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        let err = verify_relocate_at_execution(&file, &dest, &rules, true).unwrap_err();
        assert!(err.contains("boundary") || err.contains("denied"));
        let _ = std::fs::remove_dir_all(&outside);
    }
}
