//! The Organizer planner: turn an approved Zone into a reviewable plan.
//!
//! This is the orchestration layer that ties the pieces together and produces
//! the [`OrganizerPlan`] preview. It **mutates nothing**: it reads directory
//! metadata, classifies, proposes safe targets, detects conflicts, expresses
//! each proposed change as a [`Capability`], and runs every one through
//! [`policy::evaluate`]. The executor applies an approved plan through the
//! separate Execution/Audit/Undo path.
//!
//! Destination model (MVP): the first folder rule (ordered by path) that grants
//! both *create* and *move* is the destination root. Files are sorted into
//! `<destination root>/<Category>/`. Source folders are every rule granting
//! *read*. A Zone with no create+move rule yields a plan where every file is
//! skipped with [`SkipReason::NoDestination`] — deny-by-default, surfaced.

use crate::policy::{self, Capability, FolderRule, PolicyDecision};
use crate::storage::Db;
use crate::storage::zones::{get_zone, list_folder_rules};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::classifier::{LOW_CONFIDENCE, classify};
use super::conflict::Conflict;
use super::file_identity::FileIdentity;
use super::naming::{dated_prefix_with_description, deduplicate_with_description, safe_file_name};
use super::scanner::{ScannedFile, scan};

/// One proposed change, paired with its policy decision and the metadata the
/// approval UI needs to explain it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanAction {
    /// The concrete action, expressed as a policy capability.
    pub capability: Capability,
    /// What the policy engine decided about this capability.
    pub decision: PolicyDecision,
    /// The folder rule (by path) whose grant and trust level produced the
    /// decision, when one fired — so the review UI can show which
    /// user-approved boundary each proposal rests on. Optional and defaulted
    /// so plans serialized before this field existed keep deserializing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_path: Option<PathBuf>,
    /// Confidence in `0.0..=1.0` for the classification driving this action.
    /// Structural actions (folder creation) are `1.0`.
    pub confidence: f32,
    /// Human-readable justification.
    pub reason: String,
    /// Present when the proposed target collided and was de-duplicated.
    pub conflict: Option<Conflict>,
    /// File identity captured at scan time; used to detect TOCTOU swaps before execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_identity: Option<FileIdentity>,
    /// The source path of the file or folder being acted upon, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<PathBuf>,
    /// The target path of the file or folder being acted upon, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_path: Option<PathBuf>,
}

/// Why a scanned file produced no action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// The file is already in its correct category folder.
    AlreadyOrganized,
    /// The Zone grants no create+move destination, so nothing can be proposed.
    NoDestination,
}

/// A scanned file the planner intentionally left alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedFile {
    pub path: PathBuf,
    pub reason: SkipReason,
}

/// Aggregate counts for the preview header.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSummary {
    pub files_scanned: usize,
    pub create_folder: usize,
    pub move_file: usize,
    pub rename_file: usize,
    pub conflicts: usize,
    pub low_confidence: usize,
    pub denied: usize,
    pub skipped: usize,
}

/// The reviewable output of the planner. Carries actions (each with a policy
/// decision), skipped files, and a summary. Nothing here has been executed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganizerPlan {
    pub zone_id: String,
    pub actions: Vec<PlanAction>,
    pub skipped: Vec<SkippedFile>,
    pub summary: PlanSummary,
}

/// Load the Zone's folder rules from storage and build a plan.
pub fn plan_zone(conn: &Db, zone_id: &str) -> anyhow::Result<OrganizerPlan> {
    let rename_dated = get_zone(conn, zone_id)?.is_some_and(|z| z.rename_dated);
    let rules = list_folder_rules(conn, zone_id)?;
    Ok(plan_with_rules_and_options(zone_id, &rules, rename_dated))
}

/// Scan readable source folders in a Zone and return file metadata (read-only).
pub fn scan_zone_files(conn: &Db, zone_id: &str) -> anyhow::Result<Vec<ScannedFile>> {
    let rules = list_folder_rules(conn, zone_id)?;
    Ok(scan_sources(&rules))
}

/// Build a plan from an explicit set of folder rules (no storage dependency).
///
/// Exposed separately so the planner is fully testable without a database or
/// the UI — hand it rules and a temp directory tree and assert on the result.
pub fn plan_with_rules(zone_id: &str, rules: &[FolderRule]) -> OrganizerPlan {
    plan_with_rules_and_options(zone_id, rules, false)
}

/// Build a plan from explicit folder rules and zone options.
pub fn plan_with_rules_and_options(
    zone_id: &str,
    rules: &[FolderRule],
    rename_dated: bool,
) -> OrganizerPlan {
    let files = scan_sources(rules);
    let mut summary = PlanSummary {
        files_scanned: files.len(),
        ..PlanSummary::default()
    };

    let Some(dest_root) = destination_root(rules) else {
        // No create+move destination: deny-by-default, every file is skipped.
        let skipped = files
            .into_iter()
            .map(|f| SkippedFile {
                path: f.path,
                reason: SkipReason::NoDestination,
            })
            .collect::<Vec<_>>();
        summary.skipped = skipped.len();
        return OrganizerPlan {
            zone_id: zone_id.to_string(),
            actions: Vec::new(),
            skipped,
            summary,
        };
    };

    let mut actions = Vec::new();
    let mut skipped = Vec::new();
    // Per-directory name bookkeeping so two files never silently map to the
    // same target. Seeded lazily from any names already on disk.
    let mut names_by_dir: HashMap<PathBuf, DirNames> = HashMap::new();
    // Destination folders we have already emitted a CreateFolder for.
    let mut created_dirs: HashSet<PathBuf> = HashSet::new();

    for file in files {
        let class = classify(&file);
        let target_dir = dest_root.join(class.category.folder_name());
        let (proposed_name, date_description) = if rename_dated {
            match file.modified.or(file.created) {
                Some(timestamp) => dated_prefix_with_description(&file.file_name, timestamp),
                None => (file.file_name.clone(), String::new()),
            }
        } else {
            (file.file_name.clone(), String::new())
        };
        let base_name = safe_file_name(&proposed_name);
        let sanitization_description = if base_name != proposed_name {
            " sanitized filename"
        } else {
            ""
        };
        let already_here = file.path.parent() == Some(target_dir.as_path());

        // A file already correctly placed and safely named needs no action.
        if already_here && base_name == file.file_name {
            skipped.push(SkippedFile {
                path: file.path,
                reason: SkipReason::AlreadyOrganized,
            });
            continue;
        }

        let dir_names = names_by_dir
            .entry(target_dir.clone())
            .or_insert_with(|| DirNames::seed(&target_dir));

        // When renaming in place, the file's own current name must not count as
        // a collision with itself.
        let own_name = if already_here {
            Some(file.file_name.clone())
        } else {
            None
        };
        let (final_name, conflict, conflict_description) =
            dir_names.resolve(&base_name, &target_dir, own_name.as_deref());
        let target = target_dir.join(&final_name);

        // Emit a CreateFolder action once per destination folder that does not
        // already exist on disk.
        if !target_dir.exists() && created_dirs.insert(target_dir.clone()) {
            let cap = Capability::CreateFolder {
                path: target_dir.clone(),
            };
            let evaluation = policy::evaluate_with_attribution(&cap, rules);
            tally_decision(&mut summary, &evaluation.decision);
            actions.push(PlanAction {
                capability: cap,
                decision: evaluation.decision,
                rule_path: evaluation.rule_path,
                confidence: 1.0,
                reason: format!("Create destination folder {}", target_dir.display()),
                conflict: None,
                source_identity: None,
                source_path: None,
                target_path: Some(target_dir.clone()),
            });
            summary.create_folder += 1;
        }

        // Same directory => rename; different directory => move.
        let cap = if already_here {
            summary.rename_file += 1;
            Capability::RenameFile {
                from: file.path.clone(),
                to: target.clone(),
            }
        } else {
            summary.move_file += 1;
            Capability::MoveFile {
                from: file.path.clone(),
                to: target.clone(),
            }
        };
        let evaluation = policy::evaluate_with_attribution(&cap, rules);
        tally_decision(&mut summary, &evaluation.decision);

        if conflict.is_some() {
            summary.conflicts += 1;
        }
        if class.confidence <= LOW_CONFIDENCE {
            summary.low_confidence += 1;
        }

        actions.push(PlanAction {
            capability: cap,
            decision: evaluation.decision,
            rule_path: evaluation.rule_path,
            confidence: class.confidence,
            reason: format!(
                "{}{}{}{}",
                class.reason, date_description, sanitization_description, conflict_description
            ),
            conflict,
            source_identity: FileIdentity::from_path(&file.path),
            source_path: Some(file.path.clone()),
            target_path: Some(target.clone()),
        });
    }

    summary.skipped = skipped.len();
    OrganizerPlan {
        zone_id: zone_id.to_string(),
        actions,
        skipped,
        summary,
    }
}

fn tally_decision(summary: &mut PlanSummary, decision: &PolicyDecision) {
    if decision.is_denied() {
        summary.denied += 1;
    }
}

/// The first rule (rules arrive ordered by path) that grants both folder
/// creation and move — the destination root files are sorted into.
fn destination_root(rules: &[FolderRule]) -> Option<PathBuf> {
    rules
        .iter()
        .find(|r| r.can_create && r.can_move)
        .map(|r| r.path.clone())
}

/// Scan every readable source folder, de-duplicate files reachable from
/// overlapping roots, and return them sorted by path.
fn scan_sources(rules: &[FolderRule]) -> Vec<ScannedFile> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut out: Vec<ScannedFile> = Vec::new();
    for rule in rules.iter().filter(|r| r.can_read) {
        for f in scan(&rule.path) {
            if seen.insert(f.path.clone()) {
                out.push(f);
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Name bookkeeping for a single destination directory.
struct DirNames {
    /// Names already present on disk in this directory.
    disk: HashSet<String>,
    /// Names this plan has already claimed for this directory.
    planned: HashSet<String>,
}

impl DirNames {
    fn seed(dir: &Path) -> Self {
        let mut disk = HashSet::new();
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                disk.insert(entry.file_name().to_string_lossy().to_string());
            }
        }
        DirNames {
            disk,
            planned: HashSet::new(),
        }
    }

    /// Choose a final, collision-free name for `base_name` in this directory,
    /// recording any conflict that forced a rename. `own_name` is the file's
    /// current name when it already lives here (so it does not collide with
    /// itself during an in-place rename).
    fn resolve(
        &mut self,
        base_name: &str,
        target_dir: &Path,
        own_name: Option<&str>,
    ) -> (String, Option<Conflict>, String) {
        let collides_on_disk = self.disk.contains(base_name) && own_name != Some(base_name);
        let collides_in_plan = self.planned.contains(base_name);

        let conflict = if collides_on_disk {
            Some(Conflict::TargetExists {
                original_target: target_dir.join(base_name).display().to_string(),
            })
        } else if collides_in_plan {
            Some(Conflict::DuplicateInPlan {
                original_target: target_dir.join(base_name).display().to_string(),
            })
        } else {
            None
        };

        // Build the set of names to avoid: everything on disk and already
        // planned, minus the file's own current name.
        let mut taken: HashSet<String> = self.disk.union(&self.planned).cloned().collect();
        if let Some(own) = own_name {
            taken.remove(own);
        }

        let (final_name, description) = deduplicate_with_description(base_name, &taken);
        self.planned.insert(final_name.clone());
        (final_name, conflict, description)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::testutil::{TempDir, tempdir};
    use crate::policy::RiskLevel;
    use crate::storage::{open_in_memory, zones};

    fn full_rule(path: &Path) -> FolderRule {
        FolderRule {
            path: path.to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
            trust: crate::policy::TrustLevel::AskFirst,
        }
    }

    /// A temp dir set up as an in-place organize Zone: one folder with full
    /// permissions, containing a few loose files.
    fn organize_in_place_fixture() -> (TempDir, Vec<FolderRule>) {
        let tmp = tempdir();
        tmp.file("report.pdf", b"a");
        tmp.file("photo.JPG", b"b");
        tmp.file("song.mp3", b"c");
        tmp.file("mystery.qzx", b"d"); // unknown extension -> low confidence
        let rules = vec![full_rule(tmp.path())];
        (tmp, rules)
    }

    #[test]
    fn plan_does_not_mutate_the_filesystem() {
        let (tmp, rules) = organize_in_place_fixture();
        let before = listing(tmp.path());
        let _plan = plan_with_rules("z", &rules);
        let after = listing(tmp.path());
        assert_eq!(
            before, after,
            "planner must not create/move/delete anything"
        );
    }

    #[test]
    fn every_action_has_a_policy_decision_and_no_delete() {
        let (_tmp, rules) = organize_in_place_fixture();
        let plan = plan_with_rules("z", &rules);
        assert!(!plan.actions.is_empty());
        for a in &plan.actions {
            // Each move/rename in-boundary requires confirmation; folder
            // creation inside the boundary is allowed.
            match &a.capability {
                Capability::DeleteFile { .. } => panic!("planner must never propose a delete"),
                Capability::MoveFile { .. } | Capability::RenameFile { .. } => {
                    assert!(matches!(
                        a.decision,
                        PolicyDecision::RequireConfirmation {
                            risk: RiskLevel::Medium,
                            ..
                        }
                    ));
                }
                Capability::CreateFolder { .. } => assert!(a.decision.is_allowed()),
                other => panic!("unexpected capability {other:?}"),
            }
        }
    }

    #[test]
    fn plan_actions_are_attributed_to_the_rule_that_fired() {
        // In-place organize: every proposed action rests on the single
        // full-permission rule, so each carries its path as attribution.
        let (tmp, rules) = organize_in_place_fixture();
        let plan = plan_with_rules("z", &rules);
        assert!(!plan.actions.is_empty());
        for action in &plan.actions {
            assert_eq!(
                action.rule_path.as_deref(),
                Some(tmp.path()),
                "action {:?} should be attributed to the covering rule",
                action.capability
            );
        }
    }

    #[test]
    fn low_confidence_files_are_flagged() {
        let (_tmp, rules) = organize_in_place_fixture();
        let plan = plan_with_rules("z", &rules);
        assert!(
            plan.summary.low_confidence >= 1,
            "the unknown-extension file should be flagged"
        );
    }

    #[test]
    fn no_create_move_rule_skips_all_with_no_destination() {
        // No rule grants both create and move, so there is no destination root
        // and every file is skipped (deny-by-default), not proposed.
        let src = tempdir();
        src.file("a.pdf", b"x");
        let rules = vec![FolderRule::read_only(src.path())];

        let plan = plan_with_rules("z", &rules);
        assert!(plan.actions.is_empty());
        assert_eq!(plan.summary.skipped, 1);
        assert!(
            plan.skipped
                .iter()
                .all(|s| s.reason == SkipReason::NoDestination)
        );
    }

    #[test]
    fn move_from_read_only_source_is_denied() {
        // The source folder is read-only (no move permission), while a separate
        // destination grants create+move. Moving a file out of the read-only
        // source crosses an unapproved boundary, so the action is denied — and
        // the plan surfaces that rather than hiding it.
        let src = tempdir();
        src.file("a.pdf", b"x");
        let dest = tempdir();
        let rules = vec![
            FolderRule::read_only(src.path()),
            full_rule(dest.path()), // destination root: create + move granted here
        ];

        let plan = plan_with_rules("z", &rules);
        let moves: Vec<&PlanAction> = plan
            .actions
            .iter()
            .filter(|a| matches!(a.capability, Capability::MoveFile { .. }))
            .collect();
        assert_eq!(moves.len(), 1);
        assert!(moves[0].decision.is_denied());
        assert!(plan.summary.denied >= 1);
        // Folder creation inside the destination is still allowed.
        assert!(
            plan.actions
                .iter()
                .any(|a| matches!(a.capability, Capability::CreateFolder { .. })
                    && a.decision.is_allowed())
        );
    }

    #[test]
    fn client_filing_preset_rule_shape_plans_approvable_moves() {
        // Pins the exact rule shape the frontend "Client filing" guided-setup
        // preset creates (organizerRunWizard in src/main.js): the source grants
        // read + move-out (a read-only source would make the policy engine deny
        // every filing move — the bug this test guards), the destination grants
        // read/create/rename/move. Moves must come back as confirmable, never
        // denied.
        let src = tempdir();
        src.file("acme-invoice.pdf", b"x");
        let dest = tempdir();
        let source_rule = FolderRule {
            path: src.path().to_path_buf(),
            can_read: true,
            can_create: false,
            can_rename: false,
            can_move: true,
            can_copy: false,
            can_delete: false,
            trust: crate::policy::TrustLevel::AskFirst,
        };
        let dest_rule = FolderRule {
            path: dest.path().to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: false,
            can_delete: false,
            trust: crate::policy::TrustLevel::AskFirst,
        };

        let plan = plan_with_rules("z", &[source_rule, dest_rule]);
        let moves: Vec<&PlanAction> = plan
            .actions
            .iter()
            .filter(|a| matches!(a.capability, Capability::MoveFile { .. }))
            .collect();
        assert_eq!(moves.len(), 1);
        assert!(
            moves[0].decision.needs_confirmation(),
            "preset moves must be approvable, got {:?}",
            moves[0].decision
        );
        assert_eq!(plan.summary.denied, 0);
    }

    #[test]
    fn conflicts_are_detected_and_resolved_without_overwrite() {
        // Two distinct files of the same category whose safe names collide should
        // produce a conflict + de-duplicated target. The collision is driven by
        // whitespace collapse (a sanitization that works cross-platform) rather
        // than illegal characters like `?`/`*`, which cannot exist in a filename
        // on Windows and so could never be written there in the first place.
        let tmp = tempdir();
        tmp.file("a/report  card.pdf", b"1"); // two spaces -> "report card.pdf"
        tmp.file("b/report card.pdf", b"2"); // one space  -> "report card.pdf"
        let rules = vec![full_rule(tmp.path())];

        let plan = plan_with_rules("z", &rules);
        let move_targets: Vec<String> = plan
            .actions
            .iter()
            .filter_map(|a| match &a.capability {
                Capability::MoveFile { to, .. } => {
                    Some(to.file_name().unwrap().to_string_lossy().to_string())
                }
                _ => None,
            })
            .collect();
        // The two collide; one keeps the safe name, the other is de-duplicated.
        assert!(move_targets.contains(&"report card.pdf".to_string()));
        assert!(move_targets.iter().any(|n| n.contains("(2)")));
        assert!(plan.summary.conflicts >= 1);
        assert!(
            plan.actions
                .iter()
                .any(|a| a.reason.contains("renamed to avoid conflict"))
        );
    }

    #[test]
    fn actions_expose_source_and_target_paths_for_review_clients() {
        let (_tmp, rules) = organize_in_place_fixture();
        let plan = plan_with_rules("z", &rules);

        for action in plan.actions {
            match action.capability {
                Capability::CreateFolder { path } => {
                    assert!(action.source_path.is_none());
                    assert_eq!(action.target_path.as_deref(), Some(path.as_path()));
                }
                Capability::MoveFile { from, to } | Capability::RenameFile { from, to } => {
                    assert_eq!(action.source_path.as_deref(), Some(from.as_path()));
                    assert_eq!(action.target_path.as_deref(), Some(to.as_path()));
                }
                other => panic!("unexpected capability {other:?}"),
            }
        }
    }

    #[test]
    fn legacy_plan_action_without_explicit_paths_still_deserializes() {
        let action: PlanAction = serde_json::from_value(serde_json::json!({
            "capability": {
                "kind": "create_folder",
                "path": "/tmp/ghost-legacy"
            },
            "decision": {
                "decision": "allow"
            },
            "confidence": 1.0,
            "reason": "legacy preview",
            "conflict": null,
            "source_identity": null
        }))
        .unwrap();

        assert!(action.source_path.is_none());
        assert!(action.target_path.is_none());
    }

    #[test]
    fn already_organized_files_are_skipped() {
        let tmp = tempdir();
        // A file already in the correct category folder with a safe name.
        tmp.file("Documents/done.pdf", b"x");
        let rules = vec![full_rule(tmp.path())];
        let plan = plan_with_rules("z", &rules);
        assert!(
            plan.skipped
                .iter()
                .any(|s| s.reason == SkipReason::AlreadyOrganized
                    && s.path.ends_with("Documents/done.pdf"))
        );
    }

    #[test]
    fn plan_zone_loads_rules_from_storage() {
        let tmp = tempdir();
        tmp.file("notes.txt", b"x");
        let conn = open_in_memory().unwrap();
        let zone = zones::create_zone(
            &conn,
            "School",
            None,
            crate::policy::DefaultDecision::Ask,
            false,
        )
        .unwrap();
        zones::add_folder_rule(&conn, &zone.id, &full_rule(tmp.path())).unwrap();

        let plan = plan_zone(&conn, &zone.id).unwrap();
        assert_eq!(plan.zone_id, zone.id);
        assert_eq!(plan.summary.files_scanned, 1);
        // notes.txt -> Documents, in-boundary move requires confirmation.
        assert_eq!(plan.summary.move_file, 1);
        assert!(plan.actions.iter().any(|a| a.decision.needs_confirmation()));
    }

    #[test]
    fn rename_dated_off_keeps_original_filename() {
        let tmp = tempdir();
        tmp.file("acme-invoice.pdf", b"x");
        let rules = vec![full_rule(tmp.path())];

        let plan = plan_with_rules_and_options("z", &rules, false);
        let target = move_target_name(&plan).unwrap();
        assert_eq!(target, "acme-invoice.pdf");
    }

    #[test]
    fn rename_dated_on_prefixes_with_file_month() {
        let tmp = tempdir();
        tmp.file("acme-invoice.pdf", b"x");
        let rules = vec![full_rule(tmp.path())];

        let plan = plan_with_rules_and_options("z", &rules, true);
        let target = move_target_name(&plan).unwrap();
        let first_seven = target.get(0..7).unwrap_or("");
        assert!(
            first_seven.chars().enumerate().all(|(idx, ch)| {
                if idx == 4 {
                    ch == '-'
                } else {
                    ch.is_ascii_digit()
                }
            }),
            "expected YYYY-MM prefix, got {target}"
        );
        assert!(target.ends_with(" acme-invoice.pdf"));
        assert!(
            plan.actions
                .iter()
                .any(|a| a.reason.contains("added date prefix"))
        );
    }

    #[test]
    fn plan_zone_honors_rename_dated_setting() {
        let tmp = tempdir();
        tmp.file("acme-invoice.pdf", b"x");
        let conn = open_in_memory().unwrap();
        let zone = zones::create_zone(
            &conn,
            "Client filing",
            None,
            crate::policy::DefaultDecision::Ask,
            true,
        )
        .unwrap();
        zones::add_folder_rule(&conn, &zone.id, &full_rule(tmp.path())).unwrap();

        let plan = plan_zone(&conn, &zone.id).unwrap();
        let target = move_target_name(&plan).unwrap();
        assert!(target.ends_with(" acme-invoice.pdf"));
        assert_ne!(target, "acme-invoice.pdf");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn case_variant_files_get_distinct_targets_on_case_sensitive_fs() {
        let tmp = tempdir();
        tmp.file("report.pdf", b"lower");
        tmp.file("Report.pdf", b"upper");
        let rules = vec![full_rule(tmp.path())];
        let plan = plan_with_rules("z", &rules);

        assert_eq!(plan.summary.move_file, 2, "both case variants should move");
        let targets: Vec<String> = plan
            .actions
            .iter()
            .filter_map(|a| match &a.capability {
                Capability::MoveFile { to, .. } | Capability::RenameFile { to, .. } => {
                    Some(to.file_name()?.to_string_lossy().to_string())
                }
                _ => None,
            })
            .collect();
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&"report.pdf".to_string()));
        assert!(
            targets.contains(&"Report.pdf".to_string()),
            "case-sensitive FS should keep distinct targets, got {targets:?}"
        );
    }

    /// On case-insensitive volumes, a planned target that differs only by case
    /// from an on-disk name must be de-duplicated — never silently overwrite.
    #[test]
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn case_variant_collision_is_deduped_on_case_insensitive_fs() {
        let tmp = tempdir();
        tmp.dir("Documents");
        tmp.file("Documents/report.pdf", b"existing");
        tmp.file("Report.pdf", b"incoming");
        let rules = vec![full_rule(tmp.path())];
        let plan = plan_with_rules("z", &rules);
        let move_actions: Vec<_> = plan
            .actions
            .iter()
            .filter(|a| matches!(a.capability, Capability::MoveFile { .. }))
            .collect();
        assert_eq!(move_actions.len(), 1, "one incoming file should move");
        let target = match &move_actions[0].capability {
            Capability::MoveFile { to, .. } => {
                to.file_name().unwrap().to_string_lossy().to_string()
            }
            _ => unreachable!(),
        };
        assert!(
            target.contains("(2)") || target.eq_ignore_ascii_case("report.pdf"),
            "collision must dedupe, got {target}"
        );
    }

    /// Sorted listing of a directory tree (relative paths) for mutation checks.
    fn listing(root: &Path) -> Vec<String> {
        let mut out = Vec::new();
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    out.push(p.strip_prefix(base).unwrap().display().to_string());
                    if p.is_dir() {
                        walk(&p, base, out);
                    }
                }
            }
        }
        walk(root, root, &mut out);
        out.sort();
        out
    }

    fn move_target_name(plan: &OrganizerPlan) -> Option<String> {
        plan.actions.iter().find_map(|a| match &a.capability {
            Capability::MoveFile { to, .. } | Capability::RenameFile { to, .. } => {
                Some(to.file_name()?.to_string_lossy().to_string())
            }
            _ => None,
        })
    }
}
