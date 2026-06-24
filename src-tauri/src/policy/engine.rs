//! Deny-by-default policy evaluation for the Ghost Organizer MVP.
//!
//! The engine is pure: it takes a [`Capability`] and the active set of
//! [`FolderRule`]s and returns a [`PolicyDecision`]. It performs no IO and has
//! no knowledge of storage, so it is trivially unit-testable.

use super::capability::Capability;
use super::decision::PolicyDecision;
use super::risk::RiskLevel;
use super::zone::FolderRule;
use std::path::Path;

/// Component-aware containment: is `child` inside (or equal to) `parent`?
///
/// Uses [`Path::starts_with`], which compares whole path components, so
/// `/a/bc` is correctly NOT considered inside `/a/b` (a raw string prefix
/// check would get this wrong and leak access to sibling folders).
fn path_within(child: &Path, parent: &Path) -> bool {
    child.starts_with(parent)
}

fn deny(reason: impl Into<String>) -> PolicyDecision {
    PolicyDecision::Deny {
        reason: reason.into(),
    }
}

fn confirm(reason: impl Into<String>, risk: RiskLevel) -> PolicyDecision {
    PolicyDecision::RequireConfirmation {
        reason: reason.into(),
        risk,
    }
}

/// Is `path` covered by some rule that grants the permission tested by `granted`?
fn covered_by<'a>(
    path: &Path,
    rules: &'a [FolderRule],
    granted: impl Fn(&FolderRule) -> bool,
) -> Option<&'a FolderRule> {
    rules
        .iter()
        .find(|rule| granted(rule) && path_within(path, &rule.path))
}

/// Evaluate a two-sided file operation (move/rename): both endpoints must be
/// inside rules granting the permission, otherwise the operation crosses an
/// unapproved boundary and is denied.
fn evaluate_two_sided(
    from: &Path,
    to: &Path,
    rules: &[FolderRule],
    granted: impl Fn(&FolderRule) -> bool + Copy,
    verb: &str,
) -> PolicyDecision {
    let from_ok = covered_by(from, rules, granted).is_some();
    let to_ok = covered_by(to, rules, granted).is_some();
    if from_ok && to_ok {
        confirm(
            format!("{verb} {} -> {}", from.display(), to.display()),
            RiskLevel::Medium,
        )
    } else {
        deny(format!(
            "{verb} {} -> {} crosses an unapproved boundary",
            from.display(),
            to.display()
        ))
    }
}

/// Evaluate a capability against the active folder rules. Deny by default:
/// anything not explicitly permitted by a covering rule is refused.
pub fn evaluate(cap: &Capability, rules: &[FolderRule]) -> PolicyDecision {
    match cap {
        Capability::ReadFolder { path } => {
            if covered_by(path, rules, |r| r.can_read).is_some() {
                PolicyDecision::Allow
            } else {
                deny(format!(
                    "No approved zone grants read access to {}",
                    path.display()
                ))
            }
        }
        Capability::CreateFolder { path } => {
            if covered_by(path, rules, |r| r.can_create).is_some() {
                PolicyDecision::Allow
            } else {
                deny(format!(
                    "No approved zone grants folder creation at {}",
                    path.display()
                ))
            }
        }
        Capability::RenameFile { from, to } => {
            evaluate_two_sided(from, to, rules, |r| r.can_rename, "Rename")
        }
        Capability::MoveFile { from, to } => {
            evaluate_two_sided(from, to, rules, |r| r.can_move, "Move")
        }
        Capability::CopyFile { from, to } => {
            // A copy reads the source and writes the destination.
            let src_ok = covered_by(from, rules, |r| r.can_read).is_some();
            let dst_ok = covered_by(to, rules, |r| r.can_copy).is_some();
            if src_ok && dst_ok {
                confirm(
                    format!("Copy {} -> {}", from.display(), to.display()),
                    RiskLevel::Medium,
                )
            } else {
                deny(format!(
                    "Copy {} -> {} crosses an unapproved boundary",
                    from.display(),
                    to.display()
                ))
            }
        }
        // The MVP never deletes files (AGENTS.md non-negotiable rule).
        Capability::DeleteFile { path } => deny(format!(
            "Delete is disabled in the MVP ({})",
            path.display()
        )),
        // Everything below is outside the Organizer scope; deny by default.
        Capability::StartRecording => {
            deny("Recording is not a policy-approved capability in the Organizer MVP")
        }
        Capability::ReplayWorkflow { .. } => {
            deny("Workflow replay is not policy-approved in the Organizer MVP")
        }
        Capability::CaptureScreen => deny("Screen capture is not allowed in the Organizer MVP"),
        Capability::UseNetwork { host } => deny(format!(
            "Network access to {host} is denied in the Organizer MVP"
        )),
        Capability::GenerateWorkflowFromPrompt => {
            deny("AI generation may suggest, not execute; denied as a direct capability")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn full_rule(path: &str) -> FolderRule {
        FolderRule {
            path: PathBuf::from(path),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
        }
    }

    #[test]
    fn read_inside_allowed_folder_is_allowed() {
        let rules = vec![FolderRule::read_only("/home/u/Downloads")];
        let d = evaluate(
            &Capability::ReadFolder {
                path: PathBuf::from("/home/u/Downloads/sub"),
            },
            &rules,
        );
        assert_eq!(d, PolicyDecision::Allow);
    }

    #[test]
    fn read_outside_boundary_is_denied() {
        let rules = vec![FolderRule::read_only("/home/u/Downloads")];
        assert!(evaluate(
            &Capability::ReadFolder {
                path: PathBuf::from("/etc"),
            },
            &rules,
        )
        .is_denied());
    }

    #[test]
    fn create_requires_can_create() {
        // read-only rule does not grant creation
        let rules = vec![FolderRule::read_only("/home/u/Docs")];
        assert!(evaluate(
            &Capability::CreateFolder {
                path: PathBuf::from("/home/u/Docs/New"),
            },
            &rules,
        )
        .is_denied());
    }

    #[test]
    fn move_inside_boundary_requires_confirmation() {
        let rules = vec![full_rule("/home/u/Downloads"), full_rule("/home/u/Docs")];
        let d = evaluate(
            &Capability::MoveFile {
                from: PathBuf::from("/home/u/Downloads/a.pdf"),
                to: PathBuf::from("/home/u/Docs/a.pdf"),
            },
            &rules,
        );
        match d {
            PolicyDecision::RequireConfirmation { risk, .. } => assert_eq!(risk, RiskLevel::Medium),
            other => panic!("expected confirmation, got {other:?}"),
        }
    }

    #[test]
    fn move_outside_boundary_is_denied() {
        let rules = vec![full_rule("/home/u/Downloads")];
        assert!(evaluate(
            &Capability::MoveFile {
                from: PathBuf::from("/home/u/Downloads/a.pdf"),
                to: PathBuf::from("/tmp/a.pdf"),
            },
            &rules,
        )
        .is_denied());
    }

    #[test]
    fn delete_is_always_denied() {
        let rules = vec![full_rule("/home/u/Downloads")];
        assert!(evaluate(
            &Capability::DeleteFile {
                path: PathBuf::from("/home/u/Downloads/a.pdf"),
            },
            &rules,
        )
        .is_denied());
    }

    #[test]
    fn network_and_capture_are_denied() {
        assert!(evaluate(
            &Capability::UseNetwork {
                host: "example.com".into(),
            },
            &[],
        )
        .is_denied());
        assert!(evaluate(&Capability::CaptureScreen, &[]).is_denied());
        assert!(evaluate(&Capability::GenerateWorkflowFromPrompt, &[]).is_denied());
    }

    #[test]
    fn sibling_prefix_is_not_inside_boundary() {
        // "/a/bc" must NOT be treated as inside "/a/b".
        let rules = vec![FolderRule::read_only("/a/b")];
        assert!(evaluate(
            &Capability::ReadFolder {
                path: PathBuf::from("/a/bc"),
            },
            &rules,
        )
        .is_denied());
    }
}
