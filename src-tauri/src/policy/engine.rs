//! Deny-by-default policy evaluation for the Ghost Organizer MVP.
//!
//! The engine is pure: it takes a [`Capability`] and the active set of
//! [`FolderRule`]s and returns a [`PolicyDecision`]. It performs no IO and has
//! no knowledge of storage, so it is trivially unit-testable.

use super::capability::Capability;
use super::decision::PolicyDecision;
use super::risk::RiskLevel;
use super::zone::{FolderRule, TrustLevel};
use std::path::{Path, PathBuf};

/// A policy decision plus the rule that produced it, so callers (plan preview,
/// audit log) can show *which* user-approved boundary authorized or refused an
/// action. `rule_path` is `None` when no rule covered the request (the plain
/// deny-by-default case).
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub decision: PolicyDecision,
    pub rule_path: Option<PathBuf>,
}

impl Evaluation {
    fn new(decision: PolicyDecision, rule: Option<&FolderRule>) -> Self {
        Evaluation {
            decision,
            rule_path: rule.map(|r| r.path.clone()),
        }
    }

    fn unmatched(decision: PolicyDecision) -> Self {
        Evaluation {
            decision,
            rule_path: None,
        }
    }
}

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

/// Map a granted mutation through the trust level of the rule(s) that granted
/// it: `Automate` runs without prompting, `AskFirst` requires confirmation (the
/// historical behavior and the default), `Never` refuses outright.
fn decide_mutation(trust: TrustLevel, reason: String, never_reason: String) -> PolicyDecision {
    match trust {
        TrustLevel::Automate => PolicyDecision::Allow,
        TrustLevel::AskFirst => confirm(reason, RiskLevel::Medium),
        TrustLevel::Never => deny(never_reason),
    }
}

/// Of two matched rules, the one whose trust governs a two-sided operation:
/// the stricter side wins; on a tie the source-side rule is attributed.
fn stricter_rule<'a>(from_rule: &'a FolderRule, to_rule: &'a FolderRule) -> &'a FolderRule {
    if to_rule.trust.stricter(from_rule.trust) == to_rule.trust && to_rule.trust != from_rule.trust
    {
        to_rule
    } else {
        from_rule
    }
}

/// Evaluate a two-sided file operation (move/rename): both endpoints must be
/// inside rules granting the permission, otherwise the operation crosses an
/// unapproved boundary and is denied. When both sides are covered, the stricter
/// of the two rules' trust levels decides how much prompting is required.
fn evaluate_two_sided(
    from: &Path,
    to: &Path,
    rules: &[FolderRule],
    granted: impl Fn(&FolderRule) -> bool + Copy,
    verb: &str,
) -> Evaluation {
    let from_rule = covered_by(from, rules, granted);
    let to_rule = covered_by(to, rules, granted);
    match (from_rule, to_rule) {
        (Some(from_rule), Some(to_rule)) => {
            let governing = stricter_rule(from_rule, to_rule);
            let decision = decide_mutation(
                governing.trust,
                format!("{verb} {} -> {}", from.display(), to.display()),
                format!(
                    "{verb} {} -> {} is refused: the rule for {} is set to never",
                    from.display(),
                    to.display(),
                    governing.path.display()
                ),
            );
            Evaluation::new(decision, Some(governing))
        }
        _ => Evaluation::unmatched(deny(format!(
            "{verb} {} -> {} crosses an unapproved boundary",
            from.display(),
            to.display()
        ))),
    }
}

/// Evaluate a capability against the active folder rules. Deny by default:
/// anything not explicitly permitted by a covering rule is refused.
pub fn evaluate(cap: &Capability, rules: &[FolderRule]) -> PolicyDecision {
    evaluate_with_attribution(cap, rules).decision
}

/// Like [`evaluate`], but also names the folder rule that produced the
/// decision, so the plan preview and audit log can show which user-approved
/// boundary fired for each action.
pub fn evaluate_with_attribution(cap: &Capability, rules: &[FolderRule]) -> Evaluation {
    match cap {
        // Reads are Allow whenever granted, at every trust level: planning
        // requires reading, and the read grant itself is the scoping control.
        Capability::ReadFolder { path } => match covered_by(path, rules, |r| r.can_read) {
            Some(rule) => Evaluation::new(PolicyDecision::Allow, Some(rule)),
            None => Evaluation::unmatched(deny(format!(
                "No approved zone grants read access to {}",
                path.display()
            ))),
        },
        // Folder creation is structural and non-destructive: Allow under
        // Automate and AskFirst (the historical behavior), refused under Never.
        Capability::CreateFolder { path } => match covered_by(path, rules, |r| r.can_create) {
            Some(rule) if rule.trust == TrustLevel::Never => Evaluation::new(
                deny(format!(
                    "Folder creation at {} is refused: the rule for {} is set to never",
                    path.display(),
                    rule.path.display()
                )),
                Some(rule),
            ),
            Some(rule) => Evaluation::new(PolicyDecision::Allow, Some(rule)),
            None => Evaluation::unmatched(deny(format!(
                "No approved zone grants folder creation at {}",
                path.display()
            ))),
        },
        Capability::RenameFile { from, to } => {
            evaluate_two_sided(from, to, rules, |r| r.can_rename, "Rename")
        }
        Capability::MoveFile { from, to } => {
            evaluate_two_sided(from, to, rules, |r| r.can_move, "Move")
        }
        Capability::CopyFile { from, to } => {
            // A copy reads the source and writes the destination.
            let src_rule = covered_by(from, rules, |r| r.can_read);
            let dst_rule = covered_by(to, rules, |r| r.can_copy);
            match (src_rule, dst_rule) {
                (Some(src_rule), Some(dst_rule)) => {
                    let governing = stricter_rule(src_rule, dst_rule);
                    let decision = decide_mutation(
                        governing.trust,
                        format!("Copy {} -> {}", from.display(), to.display()),
                        format!(
                            "Copy {} -> {} is refused: the rule for {} is set to never",
                            from.display(),
                            to.display(),
                            governing.path.display()
                        ),
                    );
                    Evaluation::new(decision, Some(governing))
                }
                _ => Evaluation::unmatched(deny(format!(
                    "Copy {} -> {} crosses an unapproved boundary",
                    from.display(),
                    to.display()
                ))),
            }
        }
        // Delete is off by default (deny-by-default). It is granted only inside
        // a zone whose rule explicitly sets `can_delete`, and even then it is
        // NEVER silent: deletion is the highest-risk file op, so it always
        // requires explicit High-risk confirmation — Automate does not
        // auto-delete (that would violate the "no silent delete" non-negotiable
        // in AGENTS.md). When it does run, execution routes through the OS trash
        // (`organizer::trash`) so it stays recoverable; that executor wiring is
        // the follow-up to this gate.
        Capability::DeleteFile { path } => match covered_by(path, rules, |r| r.can_delete) {
            Some(rule) if rule.trust == TrustLevel::Never => Evaluation::new(
                deny(format!(
                    "Delete at {} is refused: the rule for {} is set to never",
                    path.display(),
                    rule.path.display()
                )),
                Some(rule),
            ),
            Some(rule) => Evaluation::new(
                confirm(
                    format!(
                        "Delete {} needs explicit high-risk approval (routed to the OS trash)",
                        path.display()
                    ),
                    RiskLevel::High,
                ),
                Some(rule),
            ),
            None => Evaluation::unmatched(deny(format!(
                "No approved zone grants delete at {} (deletes are off by default)",
                path.display()
            ))),
        },
        // Everything below is outside the Organizer file scope.
        Capability::StartRecording => Evaluation::unmatched(deny(
            "Recording is not a policy-approved capability in the Organizer MVP",
        )),
        // Whole-workflow replay still needs explicit approval (os-control).
        Capability::ReplayWorkflow { workflow_id } => Evaluation::unmatched(confirm(
            format!("Replay workflow `{workflow_id}` controls the OS and needs approval"),
            RiskLevel::High,
        )),
        Capability::OsClick {
            app,
            target_label,
            low_confidence,
            coordinate_only,
        } => {
            let where_ = match app {
                Some(a) if !a.is_empty() => format!("{target_label} in {a}"),
                _ => target_label.clone(),
            };
            if *coordinate_only || *low_confidence {
                Evaluation::unmatched(confirm(
                    format!(
                        "Click `{where_}` has weak targeting and needs careful approval before replay"
                    ),
                    RiskLevel::High,
                ))
            } else {
                Evaluation::unmatched(confirm(
                    format!("Click `{where_}` will control another app and needs approval"),
                    RiskLevel::Medium,
                ))
            }
        }
        Capability::OsType {
            app,
            secure_field,
            redacted,
        } => {
            if *secure_field {
                Evaluation::unmatched(deny(format!(
                    "Typing into a secure field{} is refused during routine replay",
                    app.as_ref().map(|a| format!(" in {a}")).unwrap_or_default()
                )))
            } else if *redacted {
                Evaluation::unmatched(confirm(
                    "Typed text was redacted at capture time; approve carefully before replaying keystrokes",
                    RiskLevel::High,
                ))
            } else {
                Evaluation::unmatched(confirm(
                    format!(
                        "Typing{} will control another app and needs approval",
                        app.as_ref().map(|a| format!(" in {a}")).unwrap_or_default()
                    ),
                    RiskLevel::Medium,
                ))
            }
        }
        Capability::OsShortcut { combo } => {
            let lower = combo.to_ascii_lowercase();
            let destructive = ["delete", "del", "backspace", "clear", "trash", "remove"]
                .iter()
                .any(|h| lower.contains(h));
            if destructive {
                Evaluation::unmatched(confirm(
                    format!("Shortcut `{combo}` looks destructive and needs high-risk approval"),
                    RiskLevel::High,
                ))
            } else {
                Evaluation::unmatched(confirm(
                    format!("Shortcut `{combo}` will control another app and needs approval"),
                    RiskLevel::Medium,
                ))
            }
        }
        Capability::OsScroll => Evaluation::unmatched(confirm(
            "Scroll will control another app and needs approval",
            RiskLevel::Low,
        )),
        // Pure pacing — no OS mutation beyond time.
        Capability::OsWait => Evaluation::unmatched(PolicyDecision::Allow),
        Capability::OsUnknown { description } => Evaluation::unmatched(deny(format!(
            "Unrecognized routine step cannot be approved: {description}"
        ))),
        Capability::CaptureScreen => {
            Evaluation::unmatched(deny("Screen capture is not allowed in the Organizer MVP"))
        }
        Capability::UseNetwork { host } => Evaluation::unmatched(deny(format!(
            "Network access to {host} is denied in the Organizer MVP"
        ))),
        Capability::GenerateWorkflowFromPrompt => Evaluation::unmatched(deny(
            "AI generation may suggest, not execute; denied as a direct capability",
        )),
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
            trust: crate::policy::TrustLevel::AskFirst,
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
        assert!(
            evaluate(
                &Capability::ReadFolder {
                    path: PathBuf::from("/etc"),
                },
                &rules,
            )
            .is_denied()
        );
    }

    #[test]
    fn create_requires_can_create() {
        // read-only rule does not grant creation
        let rules = vec![FolderRule::read_only("/home/u/Docs")];
        assert!(
            evaluate(
                &Capability::CreateFolder {
                    path: PathBuf::from("/home/u/Docs/New"),
                },
                &rules,
            )
            .is_denied()
        );
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
        assert!(
            evaluate(
                &Capability::MoveFile {
                    from: PathBuf::from("/home/u/Downloads/a.pdf"),
                    to: PathBuf::from("/tmp/a.pdf"),
                },
                &rules,
            )
            .is_denied()
        );
    }

    #[test]
    fn delete_is_always_denied() {
        let rules = vec![full_rule("/home/u/Downloads")];
        assert!(
            evaluate(
                &Capability::DeleteFile {
                    path: PathBuf::from("/home/u/Downloads/a.pdf"),
                },
                &rules,
            )
            .is_denied()
        );
    }

    #[test]
    fn network_and_capture_are_denied() {
        assert!(
            evaluate(
                &Capability::UseNetwork {
                    host: "example.com".into(),
                },
                &[],
            )
            .is_denied()
        );
        assert!(evaluate(&Capability::CaptureScreen, &[]).is_denied());
        assert!(evaluate(&Capability::GenerateWorkflowFromPrompt, &[]).is_denied());
    }

    fn full_rule_with_trust(path: &str, trust: TrustLevel) -> FolderRule {
        FolderRule {
            trust,
            ..full_rule(path)
        }
    }

    #[test]
    fn automate_rule_allows_moves_without_confirmation() {
        let rules = vec![
            full_rule_with_trust("/home/u/Downloads", TrustLevel::Automate),
            full_rule_with_trust("/home/u/Docs", TrustLevel::Automate),
        ];
        let d = evaluate(
            &Capability::MoveFile {
                from: PathBuf::from("/home/u/Downloads/a.pdf"),
                to: PathBuf::from("/home/u/Docs/a.pdf"),
            },
            &rules,
        );
        assert_eq!(d, PolicyDecision::Allow);
    }

    #[test]
    fn never_rule_denies_granted_moves() {
        let rules = vec![full_rule_with_trust("/home/u/Downloads", TrustLevel::Never)];
        let eval = evaluate_with_attribution(
            &Capability::MoveFile {
                from: PathBuf::from("/home/u/Downloads/a.pdf"),
                to: PathBuf::from("/home/u/Downloads/Documents/a.pdf"),
            },
            &rules,
        );
        assert!(eval.decision.is_denied());
        // The refusal is attributed to the never rule that fired.
        assert_eq!(eval.rule_path, Some(PathBuf::from("/home/u/Downloads")));
    }

    #[test]
    fn two_sided_op_uses_the_stricter_rule() {
        // Source automates, destination asks first: the move must still ask.
        let rules = vec![
            full_rule_with_trust("/home/u/Downloads", TrustLevel::Automate),
            full_rule_with_trust("/home/u/Docs", TrustLevel::AskFirst),
        ];
        let eval = evaluate_with_attribution(
            &Capability::MoveFile {
                from: PathBuf::from("/home/u/Downloads/a.pdf"),
                to: PathBuf::from("/home/u/Docs/a.pdf"),
            },
            &rules,
        );
        assert!(eval.decision.needs_confirmation());
        assert_eq!(eval.rule_path, Some(PathBuf::from("/home/u/Docs")));
    }

    #[test]
    fn never_rule_refuses_folder_creation_but_still_reads() {
        let rules = vec![full_rule_with_trust("/home/u/Downloads", TrustLevel::Never)];
        assert!(
            evaluate(
                &Capability::CreateFolder {
                    path: PathBuf::from("/home/u/Downloads/New"),
                },
                &rules,
            )
            .is_denied()
        );
        // Reads stay allowed at every trust level: the read grant is the scope.
        assert_eq!(
            evaluate(
                &Capability::ReadFolder {
                    path: PathBuf::from("/home/u/Downloads"),
                },
                &rules,
            ),
            PolicyDecision::Allow
        );
    }

    #[test]
    fn default_trust_keeps_historical_confirmation_behavior() {
        // full_rule() carries the default trust (AskFirst): in-boundary moves
        // must keep returning RequireConfirmation(Medium), exactly as before
        // trust levels existed.
        let rules = vec![full_rule("/home/u/Downloads")];
        match evaluate(
            &Capability::MoveFile {
                from: PathBuf::from("/home/u/Downloads/a.pdf"),
                to: PathBuf::from("/home/u/Downloads/Documents/a.pdf"),
            },
            &rules,
        ) {
            PolicyDecision::RequireConfirmation { risk, .. } => assert_eq!(risk, RiskLevel::Medium),
            other => panic!("expected confirmation, got {other:?}"),
        }
    }

    #[test]
    fn attribution_names_the_covering_rule_and_none_when_unmatched() {
        let rules = vec![FolderRule::read_only("/home/u/Downloads")];
        let hit = evaluate_with_attribution(
            &Capability::ReadFolder {
                path: PathBuf::from("/home/u/Downloads/sub"),
            },
            &rules,
        );
        assert_eq!(hit.rule_path, Some(PathBuf::from("/home/u/Downloads")));

        let miss = evaluate_with_attribution(
            &Capability::ReadFolder {
                path: PathBuf::from("/etc"),
            },
            &rules,
        );
        assert!(miss.decision.is_denied());
        assert_eq!(miss.rule_path, None);
    }

    #[test]
    fn delete_is_denied_even_under_automate() {
        let rules = vec![full_rule_with_trust(
            "/home/u/Downloads",
            TrustLevel::Automate,
        )];
        assert!(
            evaluate(
                &Capability::DeleteFile {
                    path: PathBuf::from("/home/u/Downloads/a.pdf"),
                },
                &rules,
            )
            .is_denied()
        );
    }

    #[test]
    fn delete_is_gated_by_can_delete_and_never_silent() {
        let del = |rule: FolderRule| {
            evaluate(
                &Capability::DeleteFile {
                    path: PathBuf::from("/home/u/Downloads/a.pdf"),
                },
                &[rule],
            )
        };
        let rule = |trust: TrustLevel, can_delete: bool| FolderRule {
            path: PathBuf::from("/home/u/Downloads"),
            can_read: true,
            can_create: false,
            can_rename: false,
            can_move: false,
            can_copy: false,
            can_delete,
            trust,
        };

        // Off by default: without a `can_delete` grant, denied at every trust level.
        assert!(del(rule(TrustLevel::AskFirst, false)).is_denied());
        assert!(del(rule(TrustLevel::Automate, false)).is_denied());

        // Granted + AskFirst -> High-risk confirmation.
        assert!(matches!(
            del(rule(TrustLevel::AskFirst, true)),
            PolicyDecision::RequireConfirmation {
                risk: RiskLevel::High,
                ..
            }
        ));

        // Granted + Automate -> STILL a High-risk confirmation, never a silent
        // auto-delete (the "no silent delete" non-negotiable overrides Automate).
        assert!(matches!(
            del(rule(TrustLevel::Automate, true)),
            PolicyDecision::RequireConfirmation {
                risk: RiskLevel::High,
                ..
            }
        ));

        // Granted but the zone is set to Never -> denied.
        assert!(del(rule(TrustLevel::Never, true)).is_denied());
    }

    #[test]
    fn sibling_prefix_is_not_inside_boundary() {
        // "/a/bc" must NOT be treated as inside "/a/b".
        let rules = vec![FolderRule::read_only("/a/b")];
        assert!(
            evaluate(
                &Capability::ReadFolder {
                    path: PathBuf::from("/a/bc"),
                },
                &rules,
            )
            .is_denied()
        );
    }
}
