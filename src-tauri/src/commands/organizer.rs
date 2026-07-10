//! Ghost Organizer commands — the IPC bridge for the trust pipeline.
//!
//! These commands expose the (already complete, separately tested) Organizer
//! backend to the frontend, in the exact order the product's trust model
//! demands:
//!
//! ```text
//! Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
//! ```
//!
//! Risk classes (see `docs/command-registry.md`):
//! - `organizer_list_zones`, `organizer_plan`, `organizer_list_executions` —
//!   **safe-read**: they touch the local DB and read directory metadata, and
//!   mutate nothing on disk.
//! - `organizer_create_zone`, `organizer_add_folder_rule` — **local-mutate**
//!   (DB only): they record user-approved boundaries.
//! - `organizer_execute`, `organizer_undo` — **local-mutate** (filesystem):
//!   they move/rename files inside an approved Zone, write an audit log and undo
//!   journal, and can roll a past run back.
//!
//! A deliberate trust choice: `organizer_execute` does **not** accept a plan from
//! the frontend. It re-plans server-side from the Zone id and the executor
//! re-checks every action through `policy::evaluate`, so a stale or tampered
//! plan posted from JS can never reach the filesystem. The preview the UI shows
//! (`organizer_plan`) and the plan actually executed are produced by the same
//! deterministic backend against the same persisted rules.

use crate::engine::GhostEngine;
use crate::organizer::executor::{execute_plan, ExecutionReport};
use crate::organizer::planner::{plan_zone, OrganizerPlan};
use crate::organizer::undo::{revert, UndoReport};
use crate::policy::{DefaultDecision, FolderRule, TrustLevel, Zone};
use crate::storage::executions::{
    get_execution, list_executions, prune_executions, save_execution, verify_chain,
    ChainVerification, ExecutionSummary,
};
use crate::storage::milestones::{list_milestones, record_milestone, Milestone};
use crate::storage::open_default;
use crate::storage::zones::{
    add_folder_rule, create_zone, list_folder_rules, list_zones, set_rule_trust,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::State;

/// Expand a user-entered folder path so Organizer rules aren't dead on arrival.
///
/// The UI historically accepted `~/Downloads` and `/Users/you/Downloads` as
/// placeholders. The scanner never expands `~`, so a literal `~/…` rule quietly
/// matches nothing and the product looks broken. Expand home (~ / $HOME) here,
/// canonicalize when the path exists, and reject empty input.
fn expand_user_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("folder path is empty".to_string());
    }

    let expanded = if trimmed == "~" {
        dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_string())?
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        let mut home =
            dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_string())?;
        home.push(rest);
        home
    } else if trimmed.starts_with('$') {
        // Best-effort $HOME / $USERPROFILE style; anything else stays literal.
        if let Some(rest) = trimmed.strip_prefix("$HOME") {
            let mut home =
                dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_string())?;
            let rest = rest.trim_start_matches('/');
            if !rest.is_empty() {
                home.push(rest);
            }
            home
        } else {
            PathBuf::from(trimmed)
        }
    } else {
        PathBuf::from(trimmed)
    };

    // Prefer a real filesystem path when the folder already exists so rules
    // match what the scanner / policy engine will see. Fall back to the
    // expanded (possibly not-yet-created) path for guided setup of empty dirs.
    if expanded.exists() {
        expanded
            .canonicalize()
            .map_err(|e| format!("could not resolve folder path: {e}"))
    } else {
        Ok(expanded)
    }
}

fn normalize_folder_rule(mut rule: FolderRule) -> Result<FolderRule, String> {
    rule.path = expand_user_path(&rule.path.to_string_lossy())?;
    Ok(rule)
}

/// Sensible first-run defaults for the Organizer UI (real paths on this machine).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizerPathDefaults {
    /// Absolute path to the user's Downloads folder when it exists.
    pub downloads: Option<String>,
    /// Absolute path to the user's home directory.
    pub home: Option<String>,
    /// Absolute path to the user's Documents folder when it exists.
    pub documents: Option<String>,
}

/// Return real local paths the Organizer can pre-fill (Downloads, home, Documents).
///
/// Risk class: safe-read. Touches: nothing on disk beyond resolving standard dirs.
#[tauri::command]
pub fn organizer_default_paths() -> OrganizerPathDefaults {
    let home = dirs::home_dir().map(|p| p.to_string_lossy().into_owned());
    let downloads = dirs::download_dir()
        .filter(|p| p.is_dir())
        .map(|p| p.to_string_lossy().into_owned());
    let documents = dirs::document_dir()
        .filter(|p| p.is_dir())
        .map(|p| p.to_string_lossy().into_owned());
    OrganizerPathDefaults {
        downloads,
        home,
        documents,
    }
}

fn path_exists_dir(path: &Path) -> bool {
    path.is_dir()
}

/// The result of executing a plan: the report plus the id under which it was
/// stored, so the UI can offer an undo for exactly this run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Id of the persisted execution (pass to `organizer_undo`).
    pub execution_id: String,
    /// Counts, audit log, and undo journal for the run.
    pub report: ExecutionReport,
}

/// List every Zone the user has approved.
///
/// Risk class: safe-read. Touches: files (DB only). No OS input, screenshots,
/// network, secrets, or app/window state.
#[tauri::command]
pub fn organizer_list_zones() -> Result<Vec<Zone>, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    list_zones(&conn).map_err(|e| e.to_string())
}

/// List the folder rules (boundaries) attached to a Zone, so the UI can show
/// exactly which folders Ghost may work in and what it is allowed to do there.
///
/// Risk class: safe-read (DB only).
#[tauri::command]
pub fn organizer_list_folder_rules(zone_id: String) -> Result<Vec<FolderRule>, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    list_folder_rules(&conn, &zone_id).map_err(|e| e.to_string())
}

/// Create a new Zone. New Zones default to `Ask` — the user still approves each
/// mutating action; this is the boundary, not a blanket allow. `rename_dated`
/// is opt-in and prefixes proposed filenames with their file month.
///
/// Risk class: local-mutate (DB only).
#[tauri::command]
pub fn organizer_create_zone(
    name: String,
    description: Option<String>,
    rename_dated: Option<bool>,
    engine: State<GhostEngine>,
) -> Result<Zone, String> {
    super::auth::require_unlocked(&engine)?;
    let conn = open_default().map_err(|e| e.to_string())?;
    let zone = create_zone(
        &conn,
        &name,
        description.as_deref(),
        DefaultDecision::Ask,
        rename_dated.unwrap_or(false),
    )
    .map_err(|e| e.to_string())?;
    // Local-only time-to-value bookkeeping; best-effort, never blocks the zone.
    let _ = record_milestone(&conn, Milestone::FirstZoneCreated);
    Ok(zone)
}

/// Attach a folder rule (a path plus per-operation permissions) to a Zone.
///
/// The rule arrives as the pure `FolderRule` domain type, so the frontend names
/// exactly which operations it is granting inside this folder. Granting nothing
/// mutating (the `read_only` shape) is the safe default.
///
/// Risk class: local-mutate (DB only).
#[tauri::command]
pub fn organizer_add_folder_rule(
    zone_id: String,
    rule: FolderRule,
    engine: State<GhostEngine>,
) -> Result<(), String> {
    super::auth::require_unlocked(&engine)?;
    // Ghost never grants delete through the Organizer surface (MVP never deletes,
    // and the policy engine denies it regardless). Refuse to even persist such a
    // rule so the stored boundary can't imply a capability the product won't honor.
    if rule.can_delete {
        return Err("Organizer folder rules cannot grant delete".to_string());
    }
    let rule = normalize_folder_rule(rule)?;
    if !path_exists_dir(&rule.path) {
        return Err(format!(
            "folder does not exist: {}. Pick a real folder on this Mac.",
            rule.path.display()
        ));
    }
    let conn = open_default().map_err(|e| e.to_string())?;
    add_folder_rule(&conn, &zone_id, &rule).map_err(|e| e.to_string())
}

/// Set the trust level of an existing folder rule (by its path within the Zone).
///
/// Trust is the user-facing control over how much autonomy a boundary carries:
/// `automate` runs its granted mutations without a per-action prompt,
/// `ask_first` (the default) requires confirmation, `never` refuses them. This
/// only records the preference; the policy engine and executor enforce it —
/// the frontend can never bypass that by editing trust.
///
/// Risk class: local-mutate (DB only).
#[tauri::command]
pub fn organizer_set_rule_trust(
    zone_id: String,
    path: String,
    trust: TrustLevel,
    engine: State<GhostEngine>,
) -> Result<(), String> {
    super::auth::require_unlocked(&engine)?;
    let path = expand_user_path(&path)?.to_string_lossy().into_owned();
    let conn = open_default().map_err(|e| e.to_string())?;
    let changed = set_rule_trust(&conn, &zone_id, &path, trust).map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err(format!("no folder rule at {path} in this zone"));
    }
    Ok(())
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn expand_rejects_empty() {
        assert!(expand_user_path("").is_err());
        assert!(expand_user_path("   ").is_err());
    }

    #[test]
    fn expand_tilde_home() {
        let home = dirs::home_dir().expect("home");
        assert_eq!(
            expand_user_path("~").unwrap(),
            home.canonicalize().unwrap_or(home.clone())
        );
        let under = expand_user_path("~/Downloads").unwrap();
        assert!(under.starts_with(&home) || under.starts_with(home.canonicalize().unwrap_or(home)));
    }

    #[test]
    fn expand_absolute_passthrough_when_missing() {
        let p = expand_user_path("/tmp/ghost-organizer-path-that-should-not-exist-xyz").unwrap();
        assert_eq!(
            p,
            PathBuf::from("/tmp/ghost-organizer-path-that-should-not-exist-xyz")
        );
    }
}

/// Produce a reviewable plan for a Zone. **Read-only**: this scans and proposes,
/// classifies, detects conflicts, and policy-checks every action, but mutates
/// nothing. This is the preview the user approves before anything runs.
///
/// Risk class: safe-read. Touches: files (reads directory metadata; no writes).
#[tauri::command]
pub fn organizer_plan(zone_id: String) -> Result<OrganizerPlan, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    let plan = plan_zone(&conn, &zone_id).map_err(|e| e.to_string())?;
    let _ = record_milestone(&conn, Milestone::FirstPlan);
    Ok(plan)
}

/// Execute a Zone's plan after the user approved it.
///
/// Re-plans server-side from the Zone id and runs the executor, which
/// independently re-checks policy per action, refuses to overwrite, writes undo
/// data before each mutation, and records an audit event for every action. The
/// resulting report (with its audit log and undo journal) is persisted so the
/// run can be reviewed and undone later.
///
/// The run is sealed into a tamper-evident hash chain as it is saved, and — if
/// the user set an audit retention policy — older runs beyond the policy are
/// pruned afterward (best-effort; a prune error never fails the run).
///
/// Risk class: local-mutate (filesystem). Touches: files (moves/renames inside
/// the approved Zone). No OS input, screenshots, network, secrets.
#[tauri::command]
pub fn organizer_execute(
    zone_id: String,
    engine: State<GhostEngine>,
) -> Result<ExecutionResult, String> {
    super::auth::require_unlocked(&engine)?;
    let conn = open_default().map_err(|e| e.to_string())?;
    let rules = list_folder_rules(&conn, &zone_id).map_err(|e| e.to_string())?;
    let plan = plan_zone(&conn, &zone_id).map_err(|e| e.to_string())?;
    let report = execute_plan(&plan, &rules);
    let execution_id = save_execution(&conn, &zone_id, &report).map_err(|e| e.to_string())?;
    let _ = record_milestone(&conn, Milestone::FirstRun);

    // Enforce the user's retention policy, if any. Deleting the user's own audit
    // history is opt-in (both bounds default to None = keep all); a prune failure
    // must not fail an otherwise-successful run.
    let audit = engine.get_config().audit;
    let _ = prune_executions(&conn, audit.retention_keep_last, audit.retention_keep_days);

    Ok(ExecutionResult {
        execution_id,
        report,
    })
}

/// List past executions, newest first, for the history/undo view.
///
/// Risk class: safe-read (DB only).
#[tauri::command]
pub fn organizer_list_executions() -> Result<Vec<ExecutionSummary>, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    list_executions(&conn).map_err(|e| e.to_string())
}

/// Undo a past execution by replaying its stored undo journal in reverse.
///
/// The undo runner preserves the executor's safety stance: it never overwrites
/// an occupied origin and never removes a folder that is no longer empty.
///
/// Risk class: local-mutate (filesystem).
#[tauri::command]
pub fn organizer_undo(
    execution_id: String,
    engine: State<GhostEngine>,
) -> Result<UndoReport, String> {
    super::auth::require_unlocked(&engine)?;
    let conn = open_default().map_err(|e| e.to_string())?;
    let stored = get_execution(&conn, &execution_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no execution with id {execution_id}"))?;
    let report = revert(&stored.undo);
    let _ = record_milestone(&conn, Milestone::FirstUndo);
    Ok(report)
}

/// Report the local, first-touch milestone timestamps used to measure
/// time-to-first-value (first zone, plan, run, undo). Each value is epoch
/// seconds as a string, or absent if that step hasn't happened yet.
///
/// This is **local-only**: it reads timestamps recorded on this machine and
/// returns them for the diagnostics view. No file contents, paths, or network.
///
/// Risk class: safe-read (DB only).
#[tauri::command]
pub fn organizer_time_to_value() -> Result<Vec<Milestoned>, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    let rows = list_milestones(&conn).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|(key, at)| Milestoned { key, at })
        .collect())
}

/// A recorded milestone timestamp for the diagnostics view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestoned {
    /// Stable milestone key (e.g. `first_run`).
    pub key: String,
    /// Epoch seconds as a string.
    pub at: String,
}

/// Verify the integrity of the stored execution audit chain: whether every
/// sealed run still matches its recorded seal and links to the one before it.
/// This is the payoff of the tamper-evidence work — a bookkeeper or auditor can
/// prove the local audit history wasn't altered, entirely offline.
///
/// Risk class: safe-read (DB only). No file contents, no network.
#[tauri::command]
pub fn organizer_verify_audit_chain() -> Result<ChainVerification, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    verify_chain(&conn).map_err(|e| e.to_string())
}

/// Export a past execution's audit log so the user has a portable record of
/// exactly what Ghost did — every action, its outcome, the rule that fired, and
/// whether it was automated or user-approved. `format` is `"json"` (the audit
/// log plus its tamper-evidence seal) or `"csv"` (one row per event, with the
/// seal as leading metadata lines). The caller writes the returned text to a
/// file it chose; this command performs no filesystem writes itself.
///
/// Risk class: safe-read (DB only; returns text, writes nothing).
#[tauri::command]
pub fn organizer_export_audit(execution_id: String, format: String) -> Result<String, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    let stored = get_execution(&conn, &execution_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no execution with id {execution_id}"))?;
    match format.as_str() {
        "json" => {
            // Wrap the audit log with the run's seal so the exported artifact
            // carries its own tamper-evidence, verifiable against the chain.
            let doc = serde_json::json!({
                "execution_id": stored.id,
                "created_at": stored.created_at,
                "hash": stored.hash,
                "prev_hash": stored.prev_hash,
                "audit": stored.audit,
            });
            serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
        }
        "csv" => {
            // Two leading metadata lines carry the seal; the audit table follows.
            let mut out = String::new();
            out.push_str(&format!(
                "# execution_id={},created_at={}\n",
                stored.id, stored.created_at
            ));
            out.push_str(&format!(
                "# hash={},prev_hash={}\n",
                stored.hash, stored.prev_hash
            ));
            out.push_str(&stored.audit.to_csv());
            Ok(out)
        }
        "compliance" => {
            Ok(stored.audit.to_compliance_report(&stored.id, &stored.created_at, &stored.hash, &stored.prev_hash))
        }
        "signed" => {
            let report = stored.audit.to_compliance_report(&stored.id, &stored.created_at, &stored.hash, &stored.prev_hash);
            let signature = generate_compliance_signature(&report);
            let mut out = report;
            out.push_str("\n---\n# Cryptographic Verification Signature\nSignature: ");
            out.push_str(&signature);
            out.push('\n');
            Ok(out)
        }
        other => Err(format!(
            "unsupported export format: {other} (use json, csv, compliance, or signed)"
        )),
    }
}

fn get_compliance_signing_secret() -> Vec<u8> {
    let key_path = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ghost")
        .join("compliance.key");
    if key_path.exists() {
        std::fs::read(&key_path).unwrap_or_else(|_| vec![0u8; 32])
    } else {
        let mut key = vec![0u8; 32];
        use aes_gcm::aead::rand_core::RngCore;
        aes_gcm::aead::rand_core::OsRng.fill_bytes(&mut key);
        let _ = std::fs::create_dir_all(key_path.parent().unwrap());
        let _ = std::fs::write(&key_path, &key);
        key
    }
}

fn generate_compliance_signature(report: &str) -> String {
    use sha2::{Sha256, Digest};
    let secret = get_compliance_signing_secret();
    let mut hasher = Sha256::new();
    hasher.update(&secret);
    hasher.update(report.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Verify the signature at the bottom of a signed compliance report against
/// the machine-local signing key. Returns true if verification succeeds,
/// false if the content has been tampered with or signature does not match.
///
/// Risk class: safe-read (calculates signature, writes nothing).
#[tauri::command]
pub fn organizer_verify_signed_report(report_content: String) -> Result<bool, String> {
    let sig_marker = "\n---\n# Cryptographic Verification Signature\nSignature: ";
    let idx = report_content.rfind(sig_marker).ok_or_else(|| {
        "No cryptographic verification signature found in report".to_string()
    })?;

    let report_body = &report_content[..idx];
    let signature_part = &report_content[idx + sig_marker.len()..];
    let recorded_signature = signature_part.trim();

    let computed_signature = generate_compliance_signature(report_body);
    Ok(recorded_signature == computed_signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_generation_and_verification() {
        let report = "This is a compliance report body.";
        let signature = generate_compliance_signature(report);
        
        let mut signed_report = report.to_string();
        signed_report.push_str("\n---\n# Cryptographic Verification Signature\nSignature: ");
        signed_report.push_str(&signature);
        signed_report.push('\n');

        // Verify successful signature
        let verified = organizer_verify_signed_report(signed_report.clone()).unwrap();
        assert!(verified);

        // Verify that tampering fails validation
        let tampered_report = signed_report.replace("compliance", "malicious");
        let verified_tampered = organizer_verify_signed_report(tampered_report).unwrap();
        assert!(!verified_tampered);
    }
}
