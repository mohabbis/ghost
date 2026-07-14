//! Versioned JSON-lines bridge for the native SwiftUI macOS preview.
//!
//! Swift presents and coordinates. This process remains the authority for
//! planning, policy, signed approval, filesystem mutation, audit receipts, and
//! undo. The bridge deliberately exposes coarse commands and stable identifiers
//! rather than Rust pointers or low-level filesystem handles.

use ghost_lib::action_plan::{from_organizer_plan, ActionKind, ActionPlan};
use ghost_lib::auth::AuthManager;
use ghost_lib::mcp::approval::verify_execution_token_with_hash;
use ghost_lib::mcp::{hash_organizer_plan, issue_approval_token, SignedApprovalToken};
use ghost_lib::organizer::planner::{plan_zone, OrganizerPlan};
use ghost_lib::organizer::scanner::scan;
use ghost_lib::organizer::undo::UndoReport;
use ghost_lib::organizer::undo_zone_run;
use ghost_lib::policy::{DefaultDecision, FolderRule, PolicyDecision, TrustLevel};
use ghost_lib::runtime::{build_receipt, execute_action_plan_with_progress, StepVerification};
use ghost_lib::storage::executions::{
    begin_execution, finish_execution, get_execution, list_executions, update_execution_progress,
};
use ghost_lib::storage::milestones::{record_milestone, Milestone};
use ghost_lib::storage::zones::{create_zone_with_rules, list_folder_rules, list_zones};
use ghost_lib::storage::{open_default, Db};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const PROTOCOL_VERSION: u32 = 1;
const SCAN_PREVIEW_LIMIT: usize = 200;

#[derive(Debug, Deserialize)]
struct RequestEnvelope {
    version: u32,
    request_id: String,
    command: String,
    #[serde(default)]
    payload: Value,
}

#[derive(Debug)]
struct BridgeFailure {
    code: &'static str,
    message: String,
    recoverable: bool,
}

impl BridgeFailure {
    fn new(code: &'static str, message: impl Into<String>, recoverable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            recoverable,
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_request", message, true)
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new("internal_error", message, false)
    }

    fn locked() -> Self {
        Self::new(
            "vault_locked",
            "Ghost is locked. Open Settings and unlock the local vault to continue.",
            true,
        )
    }
}

#[derive(Debug, Clone)]
struct ScanRecord {
    folder: PathBuf,
}

#[derive(Debug, Clone)]
struct PlanRecord {
    zone_id: String,
    plan_id: String,
    plan_hash: String,
    title: String,
}

struct BridgeState {
    scans: HashMap<String, ScanRecord>,
    plans: HashMap<String, PlanRecord>,
    auth: AuthManager,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            scans: HashMap::new(),
            plans: HashMap::new(),
            auth: AuthManager::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct AuthStatus {
    configured: bool,
    unlocked: bool,
}

#[derive(Debug, Serialize)]
struct WireScanFile {
    path: String,
    name: String,
    extension: Option<String>,
    size: u64,
}

#[derive(Debug, Serialize)]
struct WirePlanSummary {
    files_scanned: usize,
    total_steps: usize,
    create_folder: usize,
    move_file: usize,
    rename_file: usize,
    conflicts: usize,
    low_confidence: usize,
    denied: usize,
    needs_confirmation: usize,
    skipped: usize,
}

#[derive(Debug, Serialize)]
struct WirePlanStep {
    id: String,
    label: String,
    kind: &'static str,
    source_path: Option<String>,
    destination_path: Option<String>,
    decision: &'static str,
    decision_reason: Option<String>,
    risk: Option<String>,
    rule_path: Option<String>,
    confidence: f64,
    reason: String,
    conflict: Option<String>,
}

#[derive(Debug, Serialize)]
struct WireExecutionPlan {
    plan_id: String,
    zone_id: String,
    plan_hash: String,
    title: String,
    folder: String,
    summary: WirePlanSummary,
    steps: Vec<WirePlanStep>,
}

#[derive(Debug, Deserialize)]
struct ScanPayload {
    folder: String,
}

#[derive(Debug, Deserialize)]
struct ScanIdPayload {
    scan_id: String,
}

#[derive(Debug, Deserialize)]
struct PlanIdPayload {
    plan_id: String,
}

#[derive(Debug, Deserialize)]
struct ExecutePayload {
    plan_id: String,
    approval_token: String,
}

#[derive(Debug, Deserialize)]
struct ExecutionIdPayload {
    execution_id: String,
}

#[derive(Debug, Deserialize)]
struct PasswordPayload {
    password: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ghost_core_bridge stopped: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    let mut state = BridgeState::default();
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if read == 0 {
            return Ok(());
        }
        if line.trim().is_empty() {
            continue;
        }

        let request: RequestEnvelope = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                write_error(
                    &mut writer,
                    "unknown",
                    &BridgeFailure::invalid(format!("Malformed request JSON: {error}")),
                )
                .map_err(|e| e.to_string())?;
                continue;
            }
        };

        if request.version != PROTOCOL_VERSION {
            write_error(
                &mut writer,
                &request.request_id,
                &BridgeFailure::new(
                    "protocol_mismatch",
                    format!(
                        "Unsupported bridge protocol {}. This core supports {}.",
                        request.version, PROTOCOL_VERSION
                    ),
                    false,
                ),
            )
            .map_err(|e| e.to_string())?;
            continue;
        }

        match handle_request(&mut state, &request, &mut writer) {
            Ok(payload) => write_result(&mut writer, &request.request_id, payload)
                .map_err(|e| e.to_string())?,
            Err(error) => {
                write_error(&mut writer, &request.request_id, &error).map_err(|e| e.to_string())?
            }
        }
    }
}

fn handle_request<W: Write>(
    state: &mut BridgeState,
    request: &RequestEnvelope,
    writer: &mut W,
) -> Result<Value, BridgeFailure> {
    match request.command.as_str() {
        "handshake" => Ok(json!({
            "protocol_version": PROTOCOL_VERSION,
            "core_version": env!("CARGO_PKG_VERSION"),
            "capabilities": [
                "organizer.scan",
                "organizer.plan",
                "organizer.approval_token",
                "organizer.execute",
                "organizer.receipt",
                "organizer.undo",
                "organizer.history",
                "vault.unlock"
            ],
            "auth": auth_status(&state.auth)
        })),
        "auth_status" => serde_json::to_value(auth_status(&state.auth))
            .map_err(|e| BridgeFailure::internal(e.to_string())),
        "auth_unlock" => {
            let payload: PasswordPayload = decode_payload(&request.payload)?;
            let unlocked = state
                .auth
                .unlock(&payload.password)
                .map_err(|e| BridgeFailure::new("auth_error", e.to_string(), true))?;
            if !unlocked {
                return Err(BridgeFailure::new(
                    "wrong_password",
                    "The local vault password was not accepted.",
                    true,
                ));
            }
            serde_json::to_value(auth_status(&state.auth))
                .map_err(|e| BridgeFailure::internal(e.to_string()))
        }
        "auth_lock" => {
            state.auth.lock();
            serde_json::to_value(auth_status(&state.auth))
                .map_err(|e| BridgeFailure::internal(e.to_string()))
        }
        "scan" => scan_folder(state, &request.payload),
        "create_plan" => create_plan(state, &request.payload),
        "approve" => approve_plan(state, &request.payload),
        "execute" => execute_plan(state, &request.payload, &request.request_id, writer),
        "receipt" => execution_receipt(&request.payload),
        "undo" => undo_execution(state, &request.payload, &request.request_id, writer),
        "list_executions" => {
            let db = open_default().map_err(|e| BridgeFailure::internal(e.to_string()))?;
            serde_json::to_value(
                list_executions(&db).map_err(|e| BridgeFailure::internal(e.to_string()))?,
            )
            .map_err(|e| BridgeFailure::internal(e.to_string()))
        }
        _ => Err(BridgeFailure::invalid(format!(
            "Unknown command '{}'.",
            request.command
        ))),
    }
}

fn scan_folder(state: &mut BridgeState, raw_payload: &Value) -> Result<Value, BridgeFailure> {
    let payload: ScanPayload = decode_payload(raw_payload)?;
    let folder = canonical_folder(&payload.folder)?;
    let files = scan(&folder);
    let file_count = files.len();
    let total_bytes = files.iter().map(|file| file.size).sum::<u64>();
    let preview = files
        .iter()
        .take(SCAN_PREVIEW_LIMIT)
        .map(|file| WireScanFile {
            path: file.path.to_string_lossy().into_owned(),
            name: file.file_name.clone(),
            extension: file.extension.clone(),
            size: file.size,
        })
        .collect::<Vec<_>>();
    let scan_id = Uuid::new_v4().to_string();
    state.scans.insert(
        scan_id.clone(),
        ScanRecord {
            folder: folder.clone(),
        },
    );

    Ok(json!({
        "scan_id": scan_id,
        "folder": folder.to_string_lossy(),
        "file_count": file_count,
        "total_bytes": total_bytes,
        "files": preview,
        "truncated": file_count > SCAN_PREVIEW_LIMIT
    }))
}

fn create_plan(state: &mut BridgeState, raw_payload: &Value) -> Result<Value, BridgeFailure> {
    require_unlocked(&state.auth)?;
    let payload: ScanIdPayload = decode_payload(raw_payload)?;
    let scan = state.scans.get(&payload.scan_id).cloned().ok_or_else(|| {
        BridgeFailure::new("unknown_scan", "That scan is no longer available.", true)
    })?;

    let db = open_default().map_err(|e| BridgeFailure::internal(e.to_string()))?;
    let zone_id = find_or_create_native_zone(&db, &scan.folder)?;
    let organizer_plan = plan_zone(&db, &zone_id)
        .map_err(|e| BridgeFailure::new("planning_failed", e.to_string(), true))?;
    let plan_hash = hash_organizer_plan(&organizer_plan);
    let title = format!(
        "Organize {}",
        scan.folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| scan.folder.to_string_lossy().into_owned())
    );
    let plan_id = format!("native:{zone_id}:{}", Uuid::new_v4());

    state.plans.insert(
        plan_id.clone(),
        PlanRecord {
            zone_id: zone_id.clone(),
            plan_id: plan_id.clone(),
            plan_hash: plan_hash.clone(),
            title: title.clone(),
        },
    );

    serde_json::to_value(wire_plan(
        &organizer_plan,
        &plan_id,
        &plan_hash,
        &title,
        &scan.folder,
    ))
    .map_err(|e| BridgeFailure::internal(e.to_string()))
}

fn approve_plan(state: &mut BridgeState, raw_payload: &Value) -> Result<Value, BridgeFailure> {
    require_unlocked(&state.auth)?;
    let payload: PlanIdPayload = decode_payload(raw_payload)?;
    let record = state.plans.get(&payload.plan_id).cloned().ok_or_else(|| {
        BridgeFailure::new("unknown_plan", "That plan is no longer available.", true)
    })?;

    let db = open_default().map_err(|e| BridgeFailure::internal(e.to_string()))?;
    let current = plan_zone(&db, &record.zone_id)
        .map_err(|e| BridgeFailure::new("planning_failed", e.to_string(), true))?;
    let current_hash = hash_organizer_plan(&current);
    if current_hash != record.plan_hash {
        return Err(BridgeFailure::new(
            "plan_changed",
            "The folder changed after review. Scan and review a fresh plan before approving.",
            true,
        ));
    }

    let token = issue_approval_token(&record.plan_id, &record.plan_hash);
    let serialized_token =
        serde_json::to_string(&token).map_err(|e| BridgeFailure::internal(e.to_string()))?;
    Ok(json!({
        "plan_id": record.plan_id,
        "serialized_token": serialized_token,
        "expires_at": token.claims.expires_at.to_rfc3339()
    }))
}

fn execute_plan<W: Write>(
    state: &mut BridgeState,
    raw_payload: &Value,
    request_id: &str,
    writer: &mut W,
) -> Result<Value, BridgeFailure> {
    require_unlocked(&state.auth)?;
    let payload: ExecutePayload = decode_payload(raw_payload)?;
    let record = state.plans.get(&payload.plan_id).cloned().ok_or_else(|| {
        BridgeFailure::new("unknown_plan", "That plan is no longer available.", true)
    })?;

    let signed: SignedApprovalToken =
        serde_json::from_str(&payload.approval_token).map_err(|_| {
            BridgeFailure::new("invalid_approval", "Approval token is malformed.", true)
        })?;
    if signed.claims.plan_id != record.plan_id {
        return Err(BridgeFailure::new(
            "approval_mismatch",
            "Approval token does not match the requested plan.",
            true,
        ));
    }

    let db = open_default().map_err(|e| BridgeFailure::internal(e.to_string()))?;
    let fresh_plan = plan_zone(&db, &record.zone_id)
        .map_err(|e| BridgeFailure::new("planning_failed", e.to_string(), true))?;
    let fresh_hash = hash_organizer_plan(&fresh_plan);
    if fresh_hash != record.plan_hash {
        return Err(BridgeFailure::new(
            "plan_changed",
            "The effective plan changed after approval. Nothing was executed; review a fresh plan.",
            true,
        ));
    }

    verify_execution_token_with_hash(&payload.approval_token, &record.zone_id, Some(&fresh_hash))
        .map_err(|message| BridgeFailure::new("invalid_approval", message, true))?;

    let rules = list_folder_rules(&db, &record.zone_id)
        .map_err(|e| BridgeFailure::internal(e.to_string()))?;
    let mut action_plan = from_organizer_plan(&fresh_plan);
    action_plan.id = record.plan_id.clone();
    action_plan.title = record.title.clone();

    let execution_id = begin_execution(&db, &record.zone_id)
        .map_err(|e| BridgeFailure::internal(e.to_string()))?;
    write_event(
        writer,
        request_id,
        json!({
            "event": "execution_started",
            "execution_id": execution_id.clone(),
            "total_steps": action_plan.steps.len()
        }),
    )
    .map_err(|e| BridgeFailure::internal(e.to_string()))?;

    let total = action_plan.steps.len();
    let execution_id_for_progress = execution_id.clone();
    let runtime = execute_action_plan_with_progress(
        &action_plan,
        &rules,
        None,
        Some(execution_id.clone()),
        |partial| {
            let _ = update_execution_progress(&db, &execution_id_for_progress, partial);
            let completed = partial.applied + partial.skipped + partial.failed;
            let current_step = completed
                .checked_sub(1)
                .and_then(|index| action_plan.steps.get(index))
                .map(|step| step.label.clone());
            let _ = write_event(
                writer,
                request_id,
                json!({
                    "event": "execution_progress",
                    "execution_id": execution_id_for_progress.clone(),
                    "completed": completed,
                    "total": total,
                    "applied": partial.applied,
                    "skipped": partial.skipped,
                    "failed": partial.failed,
                    "current_step": current_step
                }),
            );
        },
    );

    finish_execution(
        &db,
        &execution_id,
        &record.zone_id,
        &runtime.report,
        Some(&runtime.receipt),
    )
    .map_err(|e| BridgeFailure::internal(e.to_string()))?;
    let _ = record_milestone(&db, Milestone::FirstRun);

    serde_json::to_value(runtime.receipt).map_err(|e| BridgeFailure::internal(e.to_string()))
}

fn execution_receipt(raw_payload: &Value) -> Result<Value, BridgeFailure> {
    let payload: ExecutionIdPayload = decode_payload(raw_payload)?;
    let db = open_default().map_err(|e| BridgeFailure::internal(e.to_string()))?;
    let stored = get_execution(&db, &payload.execution_id)
        .map_err(|e| BridgeFailure::internal(e.to_string()))?
        .ok_or_else(|| BridgeFailure::new("unknown_execution", "Execution was not found.", true))?;

    let receipt = if let Some(receipt) = stored.receipt {
        receipt
    } else {
        let verifications = stored
            .audit
            .events()
            .iter()
            .enumerate()
            .map(|(index, event)| {
                let label = format!("{:?}", event.capability);
                StepVerification::not_applicable(
                    &format!("audit-{index}"),
                    &label,
                    &format!("{:?}", event.outcome),
                )
            })
            .collect::<Vec<_>>();
        let report = ghost_lib::organizer::executor::ExecutionReport {
            applied: stored.applied,
            skipped: stored.skipped,
            failed: stored.failed,
            audit: stored.audit.clone(),
            undo: stored.undo.clone(),
        };
        build_receipt(
            &payload.execution_id,
            "Recovered Organizer execution",
            Some(payload.execution_id.clone()),
            &report,
            &verifications,
            &stored.created_at,
            &stored.created_at,
            false,
            None,
        )
    };

    serde_json::to_value(receipt).map_err(|e| BridgeFailure::internal(e.to_string()))
}

fn undo_execution<W: Write>(
    state: &mut BridgeState,
    raw_payload: &Value,
    request_id: &str,
    writer: &mut W,
) -> Result<Value, BridgeFailure> {
    require_unlocked(&state.auth)?;
    let payload: ExecutionIdPayload = decode_payload(raw_payload)?;
    write_event(
        writer,
        request_id,
        json!({
            "event": "undo_started",
            "execution_id": payload.execution_id.clone()
        }),
    )
    .map_err(|e| BridgeFailure::internal(e.to_string()))?;

    let db = open_default().map_err(|e| BridgeFailure::internal(e.to_string()))?;
    let report: UndoReport = undo_zone_run(&db, &payload.execution_id)
        .map_err(|e| BridgeFailure::new("undo_failed", e, true))?;
    serde_json::to_value(report).map_err(|e| BridgeFailure::internal(e.to_string()))
}

fn auth_status(auth: &AuthManager) -> AuthStatus {
    AuthStatus {
        configured: auth.is_configured(),
        unlocked: auth.is_unlocked(),
    }
}

fn require_unlocked(auth: &AuthManager) -> Result<(), BridgeFailure> {
    if auth.is_configured() && !auth.is_unlocked() {
        return Err(BridgeFailure::locked());
    }
    Ok(())
}

fn canonical_folder(raw: &str) -> Result<PathBuf, BridgeFailure> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(BridgeFailure::invalid("Folder path is empty."));
    }
    let folder = PathBuf::from(trimmed);
    if !folder.is_dir() {
        return Err(BridgeFailure::new(
            "folder_unavailable",
            format!(
                "Folder does not exist or is not readable: {}",
                folder.display()
            ),
            true,
        ));
    }
    folder.canonicalize().map_err(|e| {
        BridgeFailure::new(
            "folder_unavailable",
            format!("Could not resolve {}: {e}", folder.display()),
            true,
        )
    })
}

fn find_or_create_native_zone(db: &Db, folder: &Path) -> Result<String, BridgeFailure> {
    for zone in list_zones(db).map_err(|e| BridgeFailure::internal(e.to_string()))? {
        let rules =
            list_folder_rules(db, &zone.id).map_err(|e| BridgeFailure::internal(e.to_string()))?;
        if rules.len() == 1 && rule_matches_native_boundary(&rules[0], folder) {
            return Ok(zone.id);
        }
    }

    let name = format!(
        "Native Organizer — {}",
        folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Folder".to_string())
    );
    let description = format!(
        "Native macOS Organizer boundary for {}",
        folder.to_string_lossy()
    );
    let rule = FolderRule {
        path: folder.to_path_buf(),
        can_read: true,
        can_create: true,
        can_rename: true,
        can_move: true,
        can_copy: false,
        can_delete: false,
        trust: TrustLevel::AskFirst,
    };
    create_zone_with_rules(
        db,
        &name,
        Some(&description),
        DefaultDecision::Ask,
        false,
        &[rule],
    )
    .map(|zone| zone.id)
    .map_err(|e| BridgeFailure::internal(e.to_string()))
}

fn rule_matches_native_boundary(rule: &FolderRule, folder: &Path) -> bool {
    rule.path.as_path() == folder
        && rule.can_read
        && rule.can_create
        && rule.can_rename
        && rule.can_move
        && !rule.can_copy
        && !rule.can_delete
        && rule.trust == TrustLevel::AskFirst
}

fn wire_plan(
    organizer_plan: &OrganizerPlan,
    plan_id: &str,
    plan_hash: &str,
    title: &str,
    folder: &Path,
) -> WireExecutionPlan {
    let mut action_plan: ActionPlan = from_organizer_plan(organizer_plan);
    action_plan.id = plan_id.to_string();
    action_plan.title = title.to_string();

    let steps = action_plan
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let (kind, source_path, destination_path) = match &step.kind {
                ActionKind::CreateFolder { path } => (
                    "create_folder",
                    None,
                    Some(path.to_string_lossy().into_owned()),
                ),
                ActionKind::MoveFile { from, to } => (
                    "move_file",
                    Some(from.to_string_lossy().into_owned()),
                    Some(to.to_string_lossy().into_owned()),
                ),
                ActionKind::RenameFile { from, to } => (
                    "rename_file",
                    Some(from.to_string_lossy().into_owned()),
                    Some(to.to_string_lossy().into_owned()),
                ),
                ActionKind::VerifyPath { path, .. } => (
                    "verify_path",
                    Some(path.to_string_lossy().into_owned()),
                    None,
                ),
                _ => ("unsupported", None, None),
            };
            let (decision, decision_reason, risk) = match &step.decision {
                PolicyDecision::Allow => ("allow", None, None),
                PolicyDecision::Deny { reason } => ("deny", Some(reason.clone()), None),
                PolicyDecision::RequireConfirmation { reason, risk } => (
                    "require_confirmation",
                    Some(reason.clone()),
                    Some(format!("{risk:?}").to_lowercase()),
                ),
            };
            let conflict = organizer_plan
                .actions
                .get(index)
                .and_then(|action| action.conflict.as_ref())
                .map(|conflict| conflict.describe());

            WirePlanStep {
                id: step.id.clone(),
                label: step.label.clone(),
                kind,
                source_path,
                destination_path,
                decision,
                decision_reason,
                risk,
                rule_path: step
                    .rule_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                confidence: f64::from(step.confidence),
                reason: step.reason.clone(),
                conflict,
            }
        })
        .collect();

    WireExecutionPlan {
        plan_id: plan_id.to_string(),
        zone_id: organizer_plan.zone_id.clone(),
        plan_hash: plan_hash.to_string(),
        title: title.to_string(),
        folder: folder.to_string_lossy().into_owned(),
        summary: WirePlanSummary {
            files_scanned: organizer_plan.summary.files_scanned,
            total_steps: action_plan.summary.total_steps,
            create_folder: organizer_plan.summary.create_folder,
            move_file: organizer_plan.summary.move_file,
            rename_file: organizer_plan.summary.rename_file,
            conflicts: organizer_plan.summary.conflicts,
            low_confidence: organizer_plan.summary.low_confidence,
            denied: organizer_plan.summary.denied,
            needs_confirmation: action_plan.summary.needs_confirmation,
            skipped: organizer_plan.summary.skipped,
        },
        steps,
    }
}

fn decode_payload<T: for<'de> Deserialize<'de>>(payload: &Value) -> Result<T, BridgeFailure> {
    serde_json::from_value(payload.clone())
        .map_err(|error| BridgeFailure::invalid(format!("Invalid command payload: {error}")))
}

fn write_result<W: Write>(writer: &mut W, request_id: &str, payload: Value) -> io::Result<()> {
    write_envelope(
        writer,
        json!({
            "version": PROTOCOL_VERSION,
            "request_id": request_id,
            "kind": "result",
            "payload": payload
        }),
    )
}

fn write_event<W: Write>(writer: &mut W, request_id: &str, payload: Value) -> io::Result<()> {
    write_envelope(
        writer,
        json!({
            "version": PROTOCOL_VERSION,
            "request_id": request_id,
            "kind": "event",
            "payload": payload
        }),
    )
}

fn write_error<W: Write>(
    writer: &mut W,
    request_id: &str,
    error: &BridgeFailure,
) -> io::Result<()> {
    write_envelope(
        writer,
        json!({
            "version": PROTOCOL_VERSION,
            "request_id": request_id,
            "kind": "error",
            "error": {
                "code": error.code,
                "message": error.message,
                "recoverable": error.recoverable
            }
        }),
    )
}

fn write_envelope<W: Write>(writer: &mut W, value: Value) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, &value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_rule_is_mutation_bounded_and_delete_free() {
        let folder = PathBuf::from("/tmp/ghost-native");
        let rule = FolderRule {
            path: folder.clone(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: false,
            can_delete: false,
            trust: TrustLevel::AskFirst,
        };
        assert!(rule_matches_native_boundary(&rule, &folder));
        assert!(!rule.can_delete);
    }

    #[test]
    fn protocol_rejects_missing_payload_fields() {
        let error = decode_payload::<PlanIdPayload>(&json!({})).unwrap_err();
        assert_eq!(error.code, "invalid_request");
    }
}
