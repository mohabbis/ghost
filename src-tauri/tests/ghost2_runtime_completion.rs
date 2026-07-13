//! Ghost 2.0 runtime completion integration tests.

use ghost_lib::action_plan::{
    build_invoice_demo, from_organizer_plan_with_source, ActionKind, PlanSource,
};
use ghost_lib::organizer::planner::plan_with_rules;
use ghost_lib::policy::{FolderRule, TrustLevel};
use ghost_lib::runtime::execute_action_plan_with_progress;
use ghost_lib::runtime::semantic::{SemanticError, UiTarget};

fn scratch_dir() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ghost2_complete_{nanos}"));
    std::fs::create_dir_all(&path).unwrap();
    path
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
fn mcp_source_compiles_from_organizer_plan() {
    let tmp = scratch_dir();
    std::fs::write(tmp.join("a.pdf"), b"x").unwrap();
    let rules = vec![full_rule(&tmp)];
    let org = plan_with_rules("zone-mcp", &rules);
    let plan = from_organizer_plan_with_source(
        &org,
        PlanSource::Mcp {
            zone_id: "zone-mcp".into(),
        },
    );
    assert!(matches!(plan.source, PlanSource::Mcp { .. }));
    assert!(!plan.steps.is_empty());
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn demo_plan_includes_semantic_textedit_steps() {
    let tmp = scratch_dir();
    let downloads = tmp.join("Downloads");
    let finance = tmp.join("Documents");
    std::fs::create_dir_all(&downloads).unwrap();
    std::fs::write(downloads.join("invoice.pdf"), b"inv").unwrap();
    let plan = build_invoice_demo(&downloads, &finance).unwrap();
    assert!(plan
        .steps
        .iter()
        .any(|s| matches!(s.kind, ActionKind::SemanticSetValue { .. })));
    assert!(plan
        .steps
        .iter()
        .any(|s| matches!(s.kind, ActionKind::SemanticFocus { .. })));
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn semantic_helper_unavailable_skips_focus_on_headless() {
    let target = UiTarget {
        app: "TextEdit".into(),
        role: "AXTextArea".into(),
        title: None,
        fingerprint: None,
    };
    let err = ghost_lib::runtime::semantic::resolve_target(&target).unwrap_err();
    assert!(matches!(err, SemanticError::HelperUnavailable(_)));
}

#[test]
fn demo_fs_steps_still_execute_after_semantic_ui_steps() {
    let tmp = scratch_dir();
    let downloads = tmp.join("Downloads");
    let finance = tmp.join("Documents");
    std::fs::create_dir_all(&downloads).unwrap();
    std::fs::write(downloads.join("invoice_jan.pdf"), b"inv").unwrap();
    let plan = build_invoice_demo(&downloads, &finance).unwrap();
    let mut paths = std::collections::HashSet::new();
    for step in &plan.steps {
        if let ActionKind::MoveFile { from, to } | ActionKind::RenameFile { from, to } = &step.kind
        {
            paths.insert(from.parent().unwrap().to_path_buf());
            paths.insert(to.parent().unwrap().to_path_buf());
        }
        if let ActionKind::CreateFolder { path } = &step.kind {
            paths.insert(path.clone());
            if let Some(parent) = path.parent() {
                paths.insert(parent.to_path_buf());
            }
        }
    }
    paths.insert(tmp.clone());
    let rules: Vec<FolderRule> = paths.into_iter().map(|p| full_rule(&p)).collect();
    let result = execute_action_plan_with_progress(&plan, &rules, None, None, |_| {});
    assert!(result.report.applied >= 2);
    let _ = std::fs::remove_dir_all(&tmp);
}
