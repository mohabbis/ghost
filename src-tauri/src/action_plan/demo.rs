//! Build the Ghost 2.0 demonstration workflow: invoice from Downloads → Finance log.

use super::types::{ActionKind, ActionPlan, ActionStep, PlanSource};
use crate::organizer::file_identity::FileIdentity;
use crate::organizer::naming::dated_prefix;
use crate::policy::{evaluate_with_attribution, Capability};
use crate::runtime::semantic::UiTarget;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Newest invoice-like file in `downloads`, then rename, move, open TextEdit, log, save.
pub fn build_invoice_demo(downloads: &Path, finance_root: &Path) -> Result<ActionPlan, String> {
    if !downloads.is_dir() {
        return Err(format!(
            "downloads is not a directory: {}",
            downloads.display()
        ));
    }
    let invoice = find_newest_invoice(downloads)?;
    let identity = FileIdentity::from_path(&invoice).ok_or_else(|| {
        format!(
            "could not read identity for {}",
            invoice.file_name().unwrap_or_default().to_string_lossy()
        )
    })?;
    let meta = fs::metadata(&invoice).map_err(|e| e.to_string())?;
    let modified = meta.modified().unwrap_or(std::time::SystemTime::now());
    let base_name = invoice
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "invoice".into());
    let dated_name = dated_prefix(&base_name, modified);
    let finance_dir = finance_root.join("Finance").join("Invoices");
    let renamed = downloads.join(&dated_name);
    let final_path = finance_dir.join(&dated_name);

    let log_line = format!(
        "Ghost log: processed {} at {}",
        dated_name,
        chrono::Local::now().format("%Y-%m-%d %H:%M")
    );

    let mut steps = Vec::new();

    push_fs_step(
        &mut steps,
        0,
        ActionKind::CreateFolder {
            path: finance_dir.clone(),
        },
        Capability::CreateFolder {
            path: finance_dir.clone(),
        },
        "Create Finance/Invoices folder",
        None,
    );

    if invoice != renamed {
        push_fs_step(
            &mut steps,
            1,
            ActionKind::RenameFile {
                from: invoice.clone(),
                to: renamed.clone(),
            },
            Capability::RenameFile {
                from: invoice.clone(),
                to: renamed.clone(),
            },
            &format!(
                "Rename {} → {}",
                invoice.file_name().unwrap_or_default().to_string_lossy(),
                dated_name
            ),
            Some(identity.clone()),
        );
    }

    push_fs_step(
        &mut steps,
        2,
        ActionKind::MoveFile {
            from: renamed.clone(),
            to: final_path.clone(),
        },
        Capability::MoveFile {
            from: renamed.clone(),
            to: final_path.clone(),
        },
        &format!("Move {dated_name} → Finance/Invoices/"),
        Some(identity),
    );

    push_ui_step(
        &mut steps,
        3,
        ActionKind::OpenApplication {
            name: "TextEdit".into(),
        },
        Capability::OsClick {
            app: Some("TextEdit".into()),
            target_label: "open TextEdit".into(),
            low_confidence: false,
            coordinate_only: false,
        },
        "Open TextEdit",
    );

    let document_target = UiTarget {
        app: "TextEdit".into(),
        role: "AXTextArea".into(),
        title: None,
        fingerprint: None,
    };

    push_ui_step(
        &mut steps,
        4,
        ActionKind::SemanticFocus {
            target: document_target.clone(),
        },
        Capability::OsClick {
            app: Some("TextEdit".into()),
            target_label: "focus document".into(),
            low_confidence: false,
            coordinate_only: false,
        },
        "Focus document",
    );

    push_ui_step(
        &mut steps,
        5,
        ActionKind::SemanticSetValue {
            target: document_target,
            value: log_line.clone(),
        },
        Capability::OsType {
            app: Some("TextEdit".into()),
            secure_field: false,
            redacted: false,
        },
        "Insert log entry (semantic)",
    );

    push_ui_step(
        &mut steps,
        6,
        ActionKind::Shortcut {
            combo: "Cmd+S".into(),
        },
        Capability::OsShortcut {
            combo: "Cmd+S".into(),
        },
        "Save document",
    );

    push_verify_step(
        &mut steps,
        7,
        final_path.clone(),
        &format!("Verify {} in Finance folder", dated_name),
    );

    Ok(ActionPlan::new(
        Uuid::new_v4().to_string(),
        "Invoice → Finance workflow".into(),
        PlanSource::Demo,
        steps,
    ))
}

fn find_newest_invoice(downloads: &Path) -> Result<PathBuf, String> {
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in fs::read_dir(downloads).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() || path.is_symlink() {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let looks_like_invoice = name.contains("invoice")
            || name.contains("receipt")
            || name.ends_with(".pdf")
            || name.ends_with(".png")
            || name.ends_with(".jpg");
        if !looks_like_invoice {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                candidates.push((modified, path));
            }
        }
    }
    candidates
        .into_iter()
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
        .ok_or_else(|| "no invoice-like files found in Downloads".into())
}

fn push_fs_step(
    steps: &mut Vec<ActionStep>,
    index: usize,
    kind: ActionKind,
    capability: Capability,
    label: &str,
    source_identity: Option<FileIdentity>,
) {
    let evaluation = evaluate_with_attribution(&capability, &[]);
    steps.push(ActionStep {
        id: format!("demo-{index}"),
        label: label.into(),
        kind,
        capability,
        decision: evaluation.decision,
        rule_path: evaluation.rule_path,
        confidence: 1.0,
        reason: "demo workflow".into(),
        source_identity,
    });
}

fn push_ui_step(
    steps: &mut Vec<ActionStep>,
    index: usize,
    kind: ActionKind,
    capability: Capability,
    label: &str,
) {
    let evaluation = evaluate_with_attribution(&capability, &[]);
    steps.push(ActionStep {
        id: format!("demo-{index}"),
        label: label.into(),
        kind,
        capability,
        decision: evaluation.decision,
        rule_path: evaluation.rule_path,
        confidence: 1.0,
        reason: "demo workflow".into(),
        source_identity: None,
    });
}

fn push_verify_step(steps: &mut Vec<ActionStep>, index: usize, path: PathBuf, label: &str) {
    let capability = Capability::ReadFolder { path: path.clone() };
    let evaluation = evaluate_with_attribution(&capability, &[]);
    steps.push(ActionStep {
        id: format!("demo-{index}"),
        label: label.into(),
        kind: ActionKind::VerifyPath {
            path: path.clone(),
            should_exist: true,
        },
        capability,
        decision: evaluation.decision,
        rule_path: evaluation.rule_path,
        confidence: 1.0,
        reason: "post-move verification".into(),
        source_identity: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::testutil::tempdir;

    #[test]
    fn demo_plan_finds_newest_invoice() {
        let tmp = tempdir();
        let downloads = tmp.path().join("Downloads");
        let finance = tmp.path().join("Documents");
        fs::create_dir_all(&downloads).unwrap();
        fs::create_dir_all(&finance).unwrap();
        let old = downloads.join("old_invoice.pdf");
        let new = downloads.join("newest_invoice.pdf");
        fs::write(&old, b"old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&new, b"new").unwrap();

        let plan = build_invoice_demo(&downloads, &finance).unwrap();
        assert!(plan.summary.filesystem_steps >= 2);
        assert!(plan.summary.ui_steps >= 2);
        assert!(plan
            .steps
            .iter()
            .any(|s| matches!(s.kind, ActionKind::MoveFile { .. })));
    }
}
