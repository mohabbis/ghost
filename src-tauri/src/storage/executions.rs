//! redb-backed history of executed Organizer runs.
//!
//! When the executor applies an approved plan it produces an [`ExecutionReport`]
//! carrying the [`AuditLog`] (what happened) and the [`UndoJournal`] (how to
//! reverse it). Persisting both lets the UI list past runs and undo one later,
//! even across app restarts. The journal and audit log are stored verbatim as
//! JSON — both are already `serde`-serializable — so this module adds no new
//! domain logic, only durable bookkeeping.
//!
//! Runs are keyed by a synthetic auto-increment `seq` (assigned as
//! `highest existing seq + 1` inside the write transaction that inserts the
//! row — redb is single-writer, so this is race-free by construction). `seq`
//! stands in for the insertion order SQLite `rowid` used to provide: it is
//! what the tamper-evident hash chain links against, and what `verify_chain`/
//! `prune_executions`/"find the run before this one" all order by. A second
//! table indexes `id -> seq` for id-keyed lookups.

use crate::audit::{AuditLog, UndoJournal};
use crate::organizer::executor::ExecutionReport;
use crate::storage::Db;
use redb::{ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const EXECUTIONS: TableDefinition<u64, &[u8]> = TableDefinition::new("organizer_executions");
const EXECUTIONS_BY_ID: TableDefinition<&str, u64> =
    TableDefinition::new("organizer_executions_by_id");

/// Ensure both tables exist. Called once from `storage::open_default`/
/// `open_in_memory` right after the database is created.
pub(crate) fn init(write_txn: &WriteTransaction) -> anyhow::Result<()> {
    write_txn.open_table(EXECUTIONS)?;
    write_txn.open_table(EXECUTIONS_BY_ID)?;
    Ok(())
}

/// Epoch-seconds timestamp as a string, matching `zones::now_ts` bookkeeping.
fn now_ts() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

/// A lightweight row for the history list — counts without the heavy JSON blobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub id: String,
    pub zone_id: String,
    pub created_at: String,
    pub applied: usize,
    pub skipped: usize,
    pub failed: usize,
    /// Whether this run carries a tamper-evidence seal (empty for pre-V5 runs).
    /// Lets the history view show which runs are sealed without loading blobs.
    #[serde(default)]
    pub sealed: bool,
    /// False only for a run that began (`begin_execution`) but never reached
    /// `finish_execution` — almost always because the app crashed or was
    /// killed mid-run. `true` for every run written before this column
    /// existed (V6): those predate write-ahead durability entirely, so by
    /// definition they already ran to completion one way or another.
    #[serde(default = "default_true")]
    pub finished: bool,
}

fn default_true() -> bool {
    true
}

/// A fully-loaded past execution, including the records needed to undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredExecution {
    pub id: String,
    pub zone_id: String,
    pub created_at: String,
    pub applied: usize,
    pub skipped: usize,
    pub failed: usize,
    pub audit: AuditLog,
    pub undo: UndoJournal,
    /// SHA-256 seal over this run's row bytes plus the previous run's hash.
    /// Empty for rows written before the tamper-evidence migration (V5).
    #[serde(default)]
    pub hash: String,
    /// The `hash` of the previous execution in the chain (empty at genesis or
    /// for pre-V5 rows).
    #[serde(default)]
    pub prev_hash: String,
    /// See [`ExecutionSummary::finished`].
    #[serde(default = "default_true")]
    pub finished: bool,
}

/// The outcome of verifying the execution hash chain: whether every sealed run
/// still matches its recorded seal and links to the one before it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainVerification {
    /// True when every sealed run verifies and links correctly (or there are no
    /// sealed runs yet).
    pub intact: bool,
    /// How many runs carry a verifiable seal (written since the V5 migration).
    pub sealed_count: usize,
    /// How many runs predate tamper-evidence and carry no seal.
    pub unsealed_count: usize,
    /// The first run where verification failed, if any.
    pub first_break: Option<ChainBreak>,
}

/// Where and why the chain first failed to verify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainBreak {
    pub execution_id: String,
    pub reason: String,
}

/// Internal, storage-only row shape. Kept private: callers get [`StoredExecution`]
/// or [`ExecutionSummary`] instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionRow {
    id: String,
    zone_id: String,
    created_at: String,
    applied: usize,
    skipped: usize,
    failed: usize,
    audit_json: String,
    undo_json: String,
    hash: String,
    prev_hash: String,
    finished: bool,
}

fn stored_from_row(row: ExecutionRow) -> anyhow::Result<StoredExecution> {
    Ok(StoredExecution {
        id: row.id,
        zone_id: row.zone_id,
        created_at: row.created_at,
        applied: row.applied,
        skipped: row.skipped,
        failed: row.failed,
        audit: serde_json::from_str(&row.audit_json)?,
        undo: serde_json::from_str(&row.undo_json)?,
        hash: row.hash,
        prev_hash: row.prev_hash,
        finished: row.finished,
    })
}

/// Compute the SHA-256 seal (hex) for one execution row: a hash over the
/// previous run's hash plus this row's stored bytes, in a fixed, length-
/// delimited order so no two distinct rows can collide by field-boundary
/// ambiguity. Hashing the *stored* `audit_json`/`undo_json` strings (rather
/// than re-serializing the structs) means verification re-reads the same bytes
/// and needs no canonicalization.
#[allow(clippy::too_many_arguments)]
fn execution_row_hash(
    prev_hash: &str,
    id: &str,
    zone_id: &str,
    created_at: &str,
    applied: usize,
    skipped: usize,
    failed: usize,
    audit_json: &str,
    undo_json: &str,
) -> String {
    let mut hasher = Sha256::new();
    // Length-delimit every field so `("a","bc")` and `("ab","c")` never hash
    // alike. Counts go through their decimal string form for the same reason.
    for field in [
        prev_hash,
        id,
        zone_id,
        created_at,
        &applied.to_string(),
        &skipped.to_string(),
        &failed.to_string(),
        audit_json,
        undo_json,
    ] {
        hasher.update(field.len().to_le_bytes());
        hasher.update(field.as_bytes());
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Insert a row, assigning it the next `seq` and indexing its id. Used by
/// every write path (normal inserts and the one-time SQLite import).
fn insert_row(write_txn: &WriteTransaction, row: &ExecutionRow) -> anyhow::Result<u64> {
    let seq = {
        let table = write_txn.open_table(EXECUTIONS)?;
        let next = table.last()?.map(|(k, _)| k.value() + 1).unwrap_or(0);
        next
    };
    {
        let mut table = write_txn.open_table(EXECUTIONS)?;
        table.insert(seq, serde_json::to_vec(row)?.as_slice())?;
    }
    {
        let mut by_id = write_txn.open_table(EXECUTIONS_BY_ID)?;
        by_id.insert(row.id.as_str(), seq)?;
    }
    Ok(seq)
}

/// The hash of the most recently inserted row (by `seq`), optionally excluding
/// one id — used so `finish_execution` (which follows `begin_execution`,
/// already occupying the tip) can find the run *before* itself. A full scan is
/// deliberate: a local desktop audit trail realistically holds hundreds, not
/// millions, of rows, and this runs once per Organizer run, never in a loop.
fn last_hash(write_txn: &WriteTransaction, exclude_id: Option<&str>) -> anyhow::Result<String> {
    let table = write_txn.open_table(EXECUTIONS)?;
    let mut best: Option<(u64, String)> = None;
    for entry in table.iter()? {
        let (k, v) = entry?;
        let row: ExecutionRow = serde_json::from_slice(v.value())?;
        if Some(row.id.as_str()) == exclude_id {
            continue;
        }
        let seq = k.value();
        if best.as_ref().is_none_or(|(s, _)| seq > *s) {
            best = Some((seq, row.hash));
        }
    }
    Ok(best.map(|(_, h)| h).unwrap_or_default())
}

fn seq_for_id(write_txn: &WriteTransaction, id: &str) -> anyhow::Result<u64> {
    let by_id = write_txn.open_table(EXECUTIONS_BY_ID)?;
    let seq = by_id
        .get(id)?
        .map(|g| g.value())
        .ok_or_else(|| anyhow::anyhow!("no execution with id {id}"))?;
    Ok(seq)
}

fn read_row(write_txn: &WriteTransaction, seq: u64) -> anyhow::Result<ExecutionRow> {
    let table = write_txn.open_table(EXECUTIONS)?;
    let guard = table
        .get(seq)?
        .ok_or_else(|| anyhow::anyhow!("execution row missing for seq {seq}"))?;
    Ok(serde_json::from_slice(guard.value())?)
}

fn write_row(write_txn: &WriteTransaction, seq: u64, row: &ExecutionRow) -> anyhow::Result<()> {
    let mut table = write_txn.open_table(EXECUTIONS)?;
    table.insert(seq, serde_json::to_vec(row)?.as_slice())?;
    Ok(())
}

/// Load, mutate, and store back the row for `id`.
fn update_row(
    write_txn: &WriteTransaction,
    id: &str,
    f: impl FnOnce(&mut ExecutionRow),
) -> anyhow::Result<ExecutionRow> {
    let seq = seq_for_id(write_txn, id)?;
    let mut row = read_row(write_txn, seq)?;
    f(&mut row);
    write_row(write_txn, seq, &row)?;
    Ok(row)
}

/// Persist an execution report, returning the generated id.
pub fn save_execution(db: &Db, zone_id: &str, report: &ExecutionReport) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    let ts = now_ts();
    let audit_json = serde_json::to_string(&report.audit)?;
    let undo_json = serde_json::to_string(&report.undo)?;

    let write_txn = db.db.begin_write()?;
    let prev_hash = last_hash(&write_txn, None)?;
    let hash = execution_row_hash(
        &prev_hash,
        &id,
        zone_id,
        &ts,
        report.applied,
        report.skipped,
        report.failed,
        &audit_json,
        &undo_json,
    );
    let row = ExecutionRow {
        id: id.clone(),
        zone_id: zone_id.to_string(),
        created_at: ts,
        applied: report.applied,
        skipped: report.skipped,
        failed: report.failed,
        audit_json,
        undo_json,
        hash,
        prev_hash,
        finished: true,
    };
    insert_row(&write_txn, &row)?;
    write_txn.commit()?;
    Ok(id)
}

/// Begin a write-ahead-durable execution: insert a row *before* the executor
/// touches the filesystem, so a crash before the very first action still
/// leaves a discoverable (if empty) record rather than nothing at all.
/// `finished = false` until [`finish_execution`] runs — see
/// [`find_unfinished_execution`]. Deliberately unsealed at this point
/// (`hash`/`prev_hash` empty): sealing needs the run's *final* content, which
/// doesn't exist yet.
pub fn begin_execution(db: &Db, zone_id: &str) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    let row = ExecutionRow {
        id: id.clone(),
        zone_id: zone_id.to_string(),
        created_at: now_ts(),
        applied: 0,
        skipped: 0,
        failed: 0,
        audit_json: "[]".to_string(),
        undo_json: "[]".to_string(),
        hash: String::new(),
        prev_hash: String::new(),
        finished: false,
    };
    let write_txn = db.db.begin_write()?;
    insert_row(&write_txn, &row)?;
    write_txn.commit()?;
    Ok(id)
}

/// Durably overwrite the report-so-far for an execution begun with
/// [`begin_execution`]. Called after every action
/// (`organizer::executor::execute_plan_with_progress`), so a crash between
/// two actions loses at most the most recent snapshot write — never the
/// whole run's undo journal. Still unsealed; sealing happens once, in
/// [`finish_execution`].
pub fn update_execution_progress(
    db: &Db,
    id: &str,
    report: &ExecutionReport,
) -> anyhow::Result<()> {
    let audit_json = serde_json::to_string(&report.audit)?;
    let undo_json = serde_json::to_string(&report.undo)?;
    let write_txn = db.db.begin_write()?;
    update_row(&write_txn, id, |row| {
        row.applied = report.applied;
        row.skipped = report.skipped;
        row.failed = report.failed;
        row.audit_json = audit_json.clone();
        row.undo_json = undo_json.clone();
    })?;
    write_txn.commit()?;
    Ok(())
}

/// Finalize a run started with [`begin_execution`]: write its final content,
/// seal it into the tamper-evident chain (same scheme as [`save_execution`]),
/// and mark it finished. Excludes this row's own id when locating the chain
/// tip, since `begin_execution` already inserted it (with an empty seal)
/// before this runs — without the exclusion, a run would (harmlessly but
/// incorrectly) chain to itself instead of the run before it.
pub fn finish_execution(
    db: &Db,
    id: &str,
    zone_id: &str,
    report: &ExecutionReport,
) -> anyhow::Result<()> {
    let audit_json = serde_json::to_string(&report.audit)?;
    let undo_json = serde_json::to_string(&report.undo)?;

    let write_txn = db.db.begin_write()?;
    let seq = seq_for_id(&write_txn, id)?;
    let created_at = read_row(&write_txn, seq)?.created_at;
    let prev_hash = last_hash(&write_txn, Some(id))?;
    let hash = execution_row_hash(
        &prev_hash,
        id,
        zone_id,
        &created_at,
        report.applied,
        report.skipped,
        report.failed,
        &audit_json,
        &undo_json,
    );
    let row = ExecutionRow {
        id: id.to_string(),
        zone_id: zone_id.to_string(),
        created_at,
        applied: report.applied,
        skipped: report.skipped,
        failed: report.failed,
        audit_json,
        undo_json,
        hash,
        prev_hash,
        finished: true,
    };
    write_row(&write_txn, seq, &row)?;
    write_txn.commit()?;
    Ok(())
}

/// Mark an execution finished without changing its content — for a run the
/// user has resolved (undone, or explicitly dismissed) without going through
/// `finish_execution`'s sealing (an interrupted run's content was already
/// durably written by the last `update_execution_progress` before the
/// crash; there is nothing further to seal).
///
/// Errors (rather than silently no-op-ing) if `id` doesn't exist — see
/// [`update_row`]/[`seq_for_id`].
pub fn mark_execution_finished(db: &Db, id: &str) -> anyhow::Result<()> {
    let write_txn = db.db.begin_write()?;
    update_row(&write_txn, id, |row| row.finished = true)?;
    write_txn.commit()?;
    Ok(())
}

/// The most recent execution that began but never reached
/// [`finish_execution`] — almost always because the app crashed or was
/// killed mid-run. `None` means the last run (if any) ended cleanly (finished
/// normally, or was already resolved via [`mark_execution_finished`]).
///
/// At most one unfinished run should exist at a time in normal operation
/// (`organizer_execute` is not reentrant within one app instance), but this
/// queries for the latest defensively rather than assuming that invariant.
pub fn find_unfinished_execution(db: &Db) -> anyhow::Result<Option<StoredExecution>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(EXECUTIONS)?;
    let mut best: Option<(u64, ExecutionRow)> = None;
    for entry in table.iter()? {
        let (k, v) = entry?;
        let row: ExecutionRow = serde_json::from_slice(v.value())?;
        if !row.finished {
            let seq = k.value();
            if best.as_ref().is_none_or(|(s, _)| seq > *s) {
                best = Some((seq, row));
            }
        }
    }
    match best {
        Some((_, row)) => Ok(Some(stored_from_row(row)?)),
        None => Ok(None),
    }
}

/// Delete executions that fall outside the retention policy, returning how many
/// distinct rows were removed. Both bounds are optional; when both are `None`
/// this is a no-op (the default — Ghost keeps all audit history unless the
/// user opts in). `keep_last` retains the most recent N runs (by `seq`, i.e.
/// insertion order); `keep_days` deletes runs whose `created_at` is older than
/// `now - days`. When both are set, a run is deleted if *either* bound would
/// drop it. Because both bounds only ever remove an oldest prefix, the
/// retained runs stay a contiguous suffix and their hash chain remains
/// internally verifiable.
pub fn prune_executions(
    db: &Db,
    keep_last: Option<usize>,
    keep_days: Option<u64>,
) -> anyhow::Result<usize> {
    let write_txn = db.db.begin_write()?;
    let mut to_delete: Vec<(u64, String)> = Vec::new();

    {
        let table = write_txn.open_table(EXECUTIONS)?;
        let mut all: Vec<(u64, ExecutionRow)> = Vec::new();
        for entry in table.iter()? {
            let (k, v) = entry?;
            let row: ExecutionRow = serde_json::from_slice(v.value())?;
            all.push((k.value(), row));
        }
        // `iter()` yields ascending by key (seq), i.e. insertion order.

        if let Some(keep) = keep_last {
            let total = all.len();
            if total > keep {
                for (seq, row) in all.iter().take(total - keep) {
                    to_delete.push((*seq, row.id.clone()));
                }
            }
        }

        if let Some(days) = keep_days {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let cutoff = now.saturating_sub(days.saturating_mul(86_400));
            for (seq, row) in &all {
                let created: u64 = row.created_at.parse().unwrap_or(u64::MAX);
                if created < cutoff && !to_delete.iter().any(|(s, _)| s == seq) {
                    to_delete.push((*seq, row.id.clone()));
                }
            }
        }
    }

    let deleted = to_delete.len();
    {
        let mut table = write_txn.open_table(EXECUTIONS)?;
        let mut by_id = write_txn.open_table(EXECUTIONS_BY_ID)?;
        for (seq, id) in &to_delete {
            table.remove(*seq)?;
            by_id.remove(id.as_str())?;
        }
    }
    write_txn.commit()?;
    Ok(deleted)
}

/// Verify the execution hash chain end to end. Walks runs in insertion order
/// (by `seq`) and, for each *sealed* run (one written since the V5 migration),
/// recomputes its hash from its stored bytes and checks that (a) it matches the
/// recorded `hash` and (b) its `prev_hash` matches the previous sealed run's
/// hash. The first sealed run is the anchor — its `prev_hash` may point at a
/// pruned or genesis entry, which is expected, not a break. Pre-V5 rows carry
/// no seal and are counted separately rather than failing the check.
pub fn verify_chain(db: &Db) -> anyhow::Result<ChainVerification> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(EXECUTIONS)?;
    let mut rows: Vec<ExecutionRow> = Vec::new();
    for entry in table.iter()? {
        let (_k, v) = entry?;
        rows.push(serde_json::from_slice(v.value())?);
    }

    let mut sealed_count = 0usize;
    let mut unsealed_count = 0usize;
    let mut first_break: Option<ChainBreak> = None;
    let mut expected_prev: Option<String> = None; // hash of the previous sealed run

    for row in &rows {
        if row.hash.is_empty() {
            // Pre-V5, unsealed row: it can't be part of the verifiable chain.
            unsealed_count += 1;
            continue;
        }
        sealed_count += 1;

        // (a) Does the row still hash to its recorded seal?
        let recomputed = execution_row_hash(
            &row.prev_hash,
            &row.id,
            &row.zone_id,
            &row.created_at,
            row.applied,
            row.skipped,
            row.failed,
            &row.audit_json,
            &row.undo_json,
        );
        if recomputed != row.hash && first_break.is_none() {
            first_break = Some(ChainBreak {
                execution_id: row.id.clone(),
                reason: "run contents no longer match their recorded seal".to_string(),
            });
        }

        // (b) Does this run link to the previous sealed run? The first sealed
        // run is the anchor; earlier links may reference a pruned entry.
        if let Some(prev) = &expected_prev {
            if &row.prev_hash != prev && first_break.is_none() {
                first_break = Some(ChainBreak {
                    execution_id: row.id.clone(),
                    reason: "run does not link to the previous run's seal".to_string(),
                });
            }
        }
        expected_prev = Some(row.hash.clone());
    }

    Ok(ChainVerification {
        intact: first_break.is_none(),
        sealed_count,
        unsealed_count,
        first_break,
    })
}

/// List past executions, newest first.
pub fn list_executions(db: &Db) -> anyhow::Result<Vec<ExecutionSummary>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(EXECUTIONS)?;
    let mut out = Vec::new();
    for entry in table.iter()? {
        let (_k, v) = entry?;
        let row: ExecutionRow = serde_json::from_slice(v.value())?;
        out.push(ExecutionSummary {
            id: row.id,
            zone_id: row.zone_id,
            created_at: row.created_at,
            applied: row.applied,
            skipped: row.skipped,
            failed: row.failed,
            sealed: !row.hash.is_empty(),
            finished: row.finished,
        });
    }
    out.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(out)
}

/// True if `created_at` (epoch-seconds text) is at or after `since_epoch_secs`.
/// A malformed timestamp is treated as "not in range" rather than erroring —
/// callers are building a best-effort export window, not validating storage.
fn created_at_since(created_at: &str, since_epoch_secs: u64) -> bool {
    created_at
        .parse::<u64>()
        .is_ok_and(|ts| ts >= since_epoch_secs)
}

/// List execution summaries created at or after `since_epoch_secs`, newest
/// first — same shape and iterate-then-sort style as [`list_executions`],
/// just with a `created_at` filter applied before sorting.
pub fn list_executions_since(
    db: &Db,
    since_epoch_secs: u64,
) -> anyhow::Result<Vec<ExecutionSummary>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(EXECUTIONS)?;
    let mut out = Vec::new();
    for entry in table.iter()? {
        let (_k, v) = entry?;
        let row: ExecutionRow = serde_json::from_slice(v.value())?;
        if !created_at_since(&row.created_at, since_epoch_secs) {
            continue;
        }
        out.push(ExecutionSummary {
            id: row.id,
            zone_id: row.zone_id,
            created_at: row.created_at,
            applied: row.applied,
            skipped: row.skipped,
            failed: row.failed,
            sealed: !row.hash.is_empty(),
            finished: row.finished,
        });
    }
    out.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(out)
}

/// Like [`list_executions_since`] but loads full rows (audit log + undo
/// journal included), for callers that need the actual per-action history —
/// e.g. building an export payload — rather than just summary counts.
pub fn list_full_executions_since(
    db: &Db,
    since_epoch_secs: u64,
) -> anyhow::Result<Vec<StoredExecution>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(EXECUTIONS)?;
    let mut out = Vec::new();
    for entry in table.iter()? {
        let (_k, v) = entry?;
        let row: ExecutionRow = serde_json::from_slice(v.value())?;
        if !created_at_since(&row.created_at, since_epoch_secs) {
            continue;
        }
        out.push(stored_from_row(row)?);
    }
    out.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(out)
}

/// Load a single past execution with its audit log and undo journal.
pub fn get_execution(db: &Db, id: &str) -> anyhow::Result<Option<StoredExecution>> {
    let read_txn = db.db.begin_read()?;
    let by_id = read_txn.open_table(EXECUTIONS_BY_ID)?;
    let Some(seq_guard) = by_id.get(id)? else {
        return Ok(None);
    };
    let seq = seq_guard.value();
    let table = read_txn.open_table(EXECUTIONS)?;
    match table.get(seq)? {
        Some(guard) => {
            let row: ExecutionRow = serde_json::from_slice(guard.value())?;
            Ok(Some(stored_from_row(row)?))
        }
        None => Ok(None),
    }
}

/// Import an execution at a known id, preserving its original insertion order
/// (used only by the one-time SQLite migration — see
/// `storage::sqlite_import` — which feeds rows in ascending legacy-rowid
/// order so `seq` assignment naturally reproduces the original hash chain
/// order).
#[allow(clippy::too_many_arguments)]
pub(crate) fn import_execution_raw(
    write_txn: &WriteTransaction,
    id: &str,
    zone_id: &str,
    created_at: &str,
    applied: usize,
    skipped: usize,
    failed: usize,
    audit_json: &str,
    undo_json: &str,
    hash: &str,
    prev_hash: &str,
    finished: bool,
) -> anyhow::Result<()> {
    let row = ExecutionRow {
        id: id.to_string(),
        zone_id: zone_id.to_string(),
        created_at: created_at.to_string(),
        applied,
        skipped,
        failed,
        audit_json: audit_json.to_string(),
        undo_json: undo_json.to_string(),
        hash: hash.to_string(),
        prev_hash: prev_hash.to_string(),
        finished,
    };
    insert_row(write_txn, &row)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::executor::{execute_plan, execute_plan_with_progress};
    use crate::organizer::planner::plan_with_rules;
    use crate::policy::FolderRule;
    use crate::storage::open_in_memory;
    use std::path::{Path, PathBuf};

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

    /// A unique scratch directory for one test, cleaned up on drop. Local to this
    /// module because the organizer's own `testutil` is private to that module.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("ghost-exec-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
        fn file(&self, name: &str, bytes: &[u8]) {
            std::fs::write(self.0.join(name), bytes).unwrap();
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Directly insert a row bypassing the normal write paths, for tests that
    /// need to simulate legacy/backdated data.
    fn insert_raw(db: &Db, row: ExecutionRow) {
        let write_txn = db.db.begin_write().unwrap();
        insert_row(&write_txn, &row).unwrap();
        write_txn.commit().unwrap();
    }

    /// Directly mutate a stored row bypassing the normal write paths, for
    /// tests that need to simulate tampering.
    fn tamper(db: &Db, id: &str, f: impl FnOnce(&mut ExecutionRow)) {
        let write_txn = db.db.begin_write().unwrap();
        update_row(&write_txn, id, f).unwrap();
        write_txn.commit().unwrap();
    }

    #[test]
    fn save_and_load_round_trips_audit_and_undo() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        tmp.file("song.mp3", b"b");
        let rules = vec![full_rule(tmp.path())];

        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);
        assert!(report.applied >= 2);

        let db = open_in_memory().unwrap();
        let id = save_execution(&db, "z", &report).unwrap();

        // Summary list reflects the run.
        let list = list_executions(&db).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(list[0].applied, report.applied);

        // Full load reconstructs the audit log and undo journal byte-for-byte.
        let loaded = get_execution(&db, &id).unwrap().unwrap();
        assert_eq!(loaded.audit, report.audit);
        assert_eq!(loaded.undo, report.undo);
        assert_eq!(loaded.zone_id, "z");

        // save_execution is the one-shot legacy path: rows it writes are
        // already finished, never surfaced as needing recovery.
        assert!(loaded.finished);
        assert!(list[0].finished);
        assert!(find_unfinished_execution(&db).unwrap().is_none());
    }

    #[test]
    fn begin_execution_writes_an_unfinished_unsealed_row() {
        let db = open_in_memory().unwrap();
        let id = begin_execution(&db, "z").unwrap();

        let loaded = get_execution(&db, &id).unwrap().unwrap();
        assert!(!loaded.finished, "a begun run is not yet finished");
        assert_eq!(loaded.hash, "", "a begun run is not yet sealed");
        assert_eq!(loaded.applied, 0);
        assert!(loaded.audit.is_empty());
        assert!(loaded.undo.is_empty());
    }

    /// The scenario `begin_execution` exists for: the app crashes between two
    /// actions. Everything durably written by `update_execution_progress`
    /// before the crash must still be there — including the undo journal for
    /// files that were actually moved — even though `finish_execution` never ran.
    #[test]
    fn update_execution_progress_survives_a_simulated_crash() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        tmp.file("song.mp3", b"b");
        let rules = vec![full_rule(tmp.path())];
        let plan = plan_with_rules("z", &rules);

        let db = open_in_memory().unwrap();
        let id = begin_execution(&db, "z").unwrap();

        // Simulate the executor's progress callback firing partway through a
        // run, then the process dying before finish_execution ever runs.
        let mut last_report = None;
        execute_plan_with_progress(&plan, &rules, |report| {
            update_execution_progress(&db, &id, report).unwrap();
            last_report = Some(report.clone());
        });
        let last_report = last_report.expect("plan had at least one action");
        // (no finish_execution call — this is the "crash" point)

        let recovered = find_unfinished_execution(&db)
            .unwrap()
            .expect("the begun-but-never-finished run must be discoverable");
        assert_eq!(recovered.id, id);
        assert!(!recovered.finished);
        assert_eq!(recovered.hash, "", "an interrupted run is never sealed");
        assert_eq!(
            recovered.undo, last_report.undo,
            "undo journal survives the crash"
        );
        assert_eq!(recovered.applied, last_report.applied);
    }

    #[test]
    fn finish_execution_seals_and_marks_finished() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let plan = plan_with_rules("z", &rules);

        let db = open_in_memory().unwrap();
        // A prior, already-sealed run establishes a chain tip to link to.
        let earlier = execute_plan(&plan_with_rules("z", &rules), &rules);
        let _ = save_execution(&db, "z", &earlier).unwrap();
        let expected_prev_hash = get_execution(&db, &list_executions(&db).unwrap()[0].id)
            .unwrap()
            .unwrap()
            .hash;

        let id = begin_execution(&db, "z").unwrap();
        let report = execute_plan(&plan, &rules);
        finish_execution(&db, &id, "z", &report).unwrap();

        let loaded = get_execution(&db, &id).unwrap().unwrap();
        assert!(loaded.finished);
        assert!(
            !loaded.hash.is_empty(),
            "finish_execution must seal the run"
        );
        assert_eq!(
            loaded.prev_hash, expected_prev_hash,
            "must chain to the run before it, not to its own pre-seal empty hash"
        );
        assert_eq!(loaded.audit, report.audit);
        assert_eq!(loaded.undo, report.undo);
        assert!(find_unfinished_execution(&db).unwrap().is_none());

        // The chain (including the WAL-style run) still verifies end to end.
        assert!(verify_chain(&db).unwrap().intact);
    }

    #[test]
    fn mark_execution_finished_resolves_without_resealing() {
        let db = open_in_memory().unwrap();
        let id = begin_execution(&db, "z").unwrap();
        assert!(find_unfinished_execution(&db).unwrap().is_some());

        mark_execution_finished(&db, &id).unwrap();

        assert!(find_unfinished_execution(&db).unwrap().is_none());
        let loaded = get_execution(&db, &id).unwrap().unwrap();
        assert!(loaded.finished);
        assert_eq!(
            loaded.hash, "",
            "resolving without finish_execution stays unsealed"
        );
    }

    #[test]
    fn get_execution_returns_none_for_unknown_id() {
        let db = open_in_memory().unwrap();
        assert!(get_execution(&db, "missing").unwrap().is_none());
    }

    #[test]
    fn list_executions_is_empty_on_a_fresh_database() {
        let db = open_in_memory().unwrap();
        assert!(list_executions(&db).unwrap().is_empty());
    }

    /// A saved run carries a non-empty seal and is reported as sealed.
    #[test]
    fn a_saved_run_is_sealed() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);

        let db = open_in_memory().unwrap();
        let id = save_execution(&db, "z", &report).unwrap();
        let loaded = get_execution(&db, &id).unwrap().unwrap();
        assert!(!loaded.hash.is_empty(), "a saved run must be sealed");
        assert_eq!(loaded.prev_hash, "", "the first run is the genesis");
        assert!(list_executions(&db).unwrap()[0].sealed);
    }

    #[test]
    fn row_hash_is_deterministic_and_input_sensitive() {
        let base = execution_row_hash("", "id", "z", "100", 1, 0, 0, "[]", "[]");
        // Same inputs -> same hash.
        assert_eq!(
            base,
            execution_row_hash("", "id", "z", "100", 1, 0, 0, "[]", "[]")
        );
        // A different previous hash changes the seal (this is what chains runs).
        assert_ne!(
            base,
            execution_row_hash("prev", "id", "z", "100", 1, 0, 0, "[]", "[]")
        );
        // Tampering with the audit content changes the seal.
        assert_ne!(
            base,
            execution_row_hash("", "id", "z", "100", 1, 0, 0, "[{}]", "[]")
        );
        // Length-delimiting prevents field-boundary collisions.
        assert_ne!(
            execution_row_hash("", "ab", "c", "100", 0, 0, 0, "[]", "[]"),
            execution_row_hash("", "a", "bc", "100", 0, 0, 0, "[]", "[]")
        );
    }

    #[test]
    fn executions_form_a_verifiable_chain() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);

        let db = open_in_memory().unwrap();
        // Saving the same report several times still produces distinct, chained
        // rows (fresh id per save); the chain mechanics are what we're testing.
        for _ in 0..3 {
            save_execution(&db, "z", &report).unwrap();
        }

        let v = verify_chain(&db).unwrap();
        assert!(v.intact, "a freshly written chain must verify");
        assert_eq!(v.sealed_count, 3);
        assert_eq!(v.unsealed_count, 0);
        assert!(v.first_break.is_none());
    }

    #[test]
    fn tampering_with_a_stored_run_breaks_verification() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);

        let db = open_in_memory().unwrap();
        let first = save_execution(&db, "z", &report).unwrap();
        save_execution(&db, "z", &report).unwrap();

        // Alter the stored audit of the first run — the seal no longer matches.
        tamper(&db, &first, |row| row.audit_json = "[]".to_string());

        let v = verify_chain(&db).unwrap();
        assert!(!v.intact, "tampering must be detectable");
        let brk = v.first_break.expect("a break must be reported");
        assert_eq!(brk.execution_id, first);
    }

    #[test]
    fn prune_keep_last_retains_recent_runs_and_chain_stays_intact() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);

        let db = open_in_memory().unwrap();
        for _ in 0..3 {
            save_execution(&db, "z", &report).unwrap();
        }

        let deleted = prune_executions(&db, Some(1), None).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(list_executions(&db).unwrap().len(), 1);
        // The retained suffix still verifies among itself.
        assert!(verify_chain(&db).unwrap().intact);
    }

    #[test]
    fn prune_keep_days_deletes_old_runs_only() {
        let db = open_in_memory().unwrap();
        // An ancient unsealed row (created ~100 days ago) and a fresh sealed one.
        let old_ts = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 100 * 86_400)
            .to_string();
        insert_raw(
            &db,
            ExecutionRow {
                id: "old".to_string(),
                zone_id: "z".to_string(),
                created_at: old_ts,
                applied: 0,
                skipped: 0,
                failed: 0,
                audit_json: "[]".to_string(),
                undo_json: "[]".to_string(),
                hash: String::new(),
                prev_hash: String::new(),
                finished: true,
            },
        );

        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);
        let fresh = save_execution(&db, "z", &report).unwrap();

        let deleted = prune_executions(&db, None, Some(30)).unwrap();
        assert_eq!(deleted, 1);
        assert!(get_execution(&db, "old").unwrap().is_none());
        assert!(get_execution(&db, &fresh).unwrap().is_some());
    }

    #[test]
    fn prune_is_a_noop_when_no_policy_is_set() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);

        let db = open_in_memory().unwrap();
        save_execution(&db, "z", &report).unwrap();
        save_execution(&db, "z", &report).unwrap();

        let deleted = prune_executions(&db, None, None).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(list_executions(&db).unwrap().len(), 2);
    }

    #[test]
    fn list_executions_since_excludes_older_runs() {
        let db = open_in_memory().unwrap();
        let old_ts = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 100 * 86_400)
            .to_string();
        insert_raw(
            &db,
            ExecutionRow {
                id: "old".to_string(),
                zone_id: "z".to_string(),
                created_at: old_ts,
                applied: 0,
                skipped: 0,
                failed: 0,
                audit_json: "[]".to_string(),
                undo_json: "[]".to_string(),
                hash: String::new(),
                prev_hash: String::new(),
                finished: true,
            },
        );

        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);
        let fresh = save_execution(&db, "z", &report).unwrap();

        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 30 * 86_400;
        let recent = list_executions_since(&db, cutoff).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, fresh);
    }

    #[test]
    fn list_full_executions_since_includes_audit_and_undo() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        tmp.file("song.mp3", b"b");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);

        let db = open_in_memory().unwrap();
        let id = save_execution(&db, "z", &report).unwrap();

        let since = 0u64;
        let full = list_full_executions_since(&db, since).unwrap();
        assert_eq!(full.len(), 1);
        assert_eq!(full[0].id, id);
        assert_eq!(full[0].audit, report.audit);
        assert_eq!(full[0].undo, report.undo);
    }

    /// Pre-V5 rows have no seal; they are counted as unsealed, and the sealed
    /// runs written after still verify.
    #[test]
    fn unsealed_legacy_rows_do_not_fail_verification() {
        let db = open_in_memory().unwrap();
        insert_raw(
            &db,
            ExecutionRow {
                id: "legacy".to_string(),
                zone_id: "z".to_string(),
                created_at: "1".to_string(),
                applied: 0,
                skipped: 0,
                failed: 0,
                audit_json: "[]".to_string(),
                undo_json: "[]".to_string(),
                hash: String::new(),
                prev_hash: String::new(),
                finished: true,
            },
        );

        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);
        save_execution(&db, "z", &report).unwrap();

        let v = verify_chain(&db).unwrap();
        assert!(v.intact);
        assert_eq!(v.unsealed_count, 1);
        assert_eq!(v.sealed_count, 1);
    }
}
