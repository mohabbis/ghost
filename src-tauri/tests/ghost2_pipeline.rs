//! Ghost 2.0 unified pipeline integration tests.

use ghost_lib::action_plan::{build_invoice_demo, from_organizer_plan, ActionPlan};
use ghost_lib::organizer::planner::plan_with_rules;
use ghost_lib::policy::{FolderRule, TrustLevel};
use ghost_lib::runtime::execute_action_plan_with_progress;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ghost2_test_{nanos}"));
    fs::create_dir_all(&path).unwrap();
    // Resolve symlinks (macOS `temp_dir()` is `/var/...` -> `/private/var/...`)
    // so the FolderRule path matches the canonicalized paths the planner emits;
    // otherwise zone containment rejects every file and the plan is empty.
    path.canonicalize().unwrap()
}

fn full_rule(path: &std::path::Path) -> FolderRule {
    FolderRule {
        path: path.to_path_buf(),
        can_read: true,
        can_create: true,
        can_rename: true,
        can_move: true,
        can_copy: true,
        can_delete: false,
        trust: TrustLevel::Automate,
    }
}

#[test]
fn organizer_plan_compiles_to_action_plan_and_executes() {
    let tmp = scratch_dir();
    fs::write(tmp.join("report.pdf"), b"pdf").unwrap();
    let rules = vec![full_rule(&tmp)];
    let org = plan_with_rules("z", &rules);
    let action_plan = from_organizer_plan(&org);
    assert!(action_plan.summary.filesystem_steps > 0);

    let result = execute_action_plan_with_progress(&action_plan, &rules, None, None, |_| {});
    assert!(result.report.applied > 0 || result.report.skipped > 0);
    assert!(!result.receipt.plan_id.is_empty());
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn demo_workflow_serializes_and_runs_fs_steps() {
    let tmp = scratch_dir();
    let downloads = tmp.join("Downloads");
    let finance = tmp.join("Documents");
    fs::create_dir_all(&downloads).unwrap();
    fs::write(downloads.join("invoice_jan.pdf"), b"inv").unwrap();

    let plan = build_invoice_demo(&downloads, &finance).unwrap();
    let json = serde_json::to_string(&plan).unwrap();
    let back: ActionPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(back.steps.len(), plan.steps.len());

    let mut paths = std::collections::HashSet::new();
    paths.insert(tmp.clone());
    paths.insert(downloads.clone());
    paths.insert(finance.clone());
    let rules: Vec<FolderRule> = paths.into_iter().map(|p| full_rule(&p)).collect();

    let result = execute_action_plan_with_progress(&plan, &rules, None, None, |_| {});
    assert!(
        result.report.applied >= 2,
        "expected rename+move at minimum"
    );
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn resource_drift_skips_stale_move() {
    let tmp = scratch_dir();
    let src = tmp.join("a.pdf");
    fs::write(&src, b"one").unwrap();
    let dst = tmp.join("moved.pdf");
    let identity = ghost_lib::organizer::file_identity::FileIdentity::from_path(&src).unwrap();
    fs::write(&src, b"much-longer-content").unwrap();

    let plan = ghost_lib::action_plan::ActionPlan::new(
        "t".into(),
        "drift".into(),
        ghost_lib::action_plan::PlanSource::Demo,
        vec![ghost_lib::action_plan::ActionStep {
            id: "m1".into(),
            label: "move".into(),
            kind: ghost_lib::action_plan::ActionKind::MoveFile {
                from: src.clone(),
                to: dst.clone(),
            },
            capability: ghost_lib::policy::Capability::MoveFile {
                from: src.clone(),
                to: dst.clone(),
            },
            decision: ghost_lib::policy::PolicyDecision::Allow,
            rule_path: None,
            confidence: 1.0,
            reason: String::new(),
            source_identity: Some(identity),
        }],
    );
    let rules = vec![full_rule(&tmp)];
    let result = execute_action_plan_with_progress(&plan, &rules, None, None, |_| {});
    assert_eq!(result.report.skipped, 1);
    let _ = fs::remove_dir_all(&tmp);
}
