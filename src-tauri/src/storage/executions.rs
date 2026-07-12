//! SQLite-backed history of executed Organizer runs.
//!
//! When the executor applies an approved plan it produces an [`ExecutionReport`]
//! carrying the [`AuditLog`] (what happened) and the [`UndoJournal`] (how to
//! reverse it). Persisting both lets the UI list past runs and undo one later,
//! even across app restarts. The journal and audit log are stored verbatim as
//! JSON columns — both are already `serde`-serializable — so this module adds no
//! new domain logic, only durable bookkeeping.

use crate::audit::{AuditLog, UndoJournal};
use crate::organizer::executor::ExecutionReport;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

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

/// Persist an execution report, returning the generated id.
pub fn save_execution(
    conn: &Connection,
    zone_id: &str,
    report: &ExecutionReport,
) -> rusqlite::Result<String> {
    let id = Uuid::new_v4().to_string();
    let ts = now_ts();
    // The audit log / undo journal are pure data; serialization cannot realistically
    // fail, but surface any error as a SQLite-compatible error rather than panicking.
    let audit_json = serde_json::to_string(&report.audit).map_err(to_sqlite_err)?;
    let undo_json = serde_json::to_string(&report.undo).map_err(to_sqlite_err)?;

    // Seal this run into the tamper-evident chain: read the current tip (highest
    // rowid = most recently inserted, the reliable insertion order — `created_at`
    // is second-granular and `id` is a random UUID) and hash over it plus this
    // row's bytes. An empty tip means this is the genesis (or the first run after
    // the V5 upgrade).
    let prev_hash: String = conn
        .query_row(
            "SELECT hash FROM organizer_executions ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();
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

    conn.execute(
        "INSERT INTO organizer_executions \
         (id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json, \
          hash, prev_hash) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            zone_id,
            ts,
            report.applied as i64,
            report.skipped as i64,
            report.failed as i64,
            audit_json,
            undo_json,
            hash,
            prev_hash,
        ],
    )?;
    Ok(id)
}

/// Begin a write-ahead-durable execution: insert a row *before* the executor
/// touches the filesystem, so a crash before the very first action still
/// leaves a discoverable (if empty) record rather than nothing at all.
/// `finished = 0` until [`finish_execution`] runs — see
/// [`find_unfinished_execution`]. Deliberately unsealed at this point
/// (`hash`/`prev_hash` empty): sealing needs the run's *final* content, which
/// doesn't exist yet.
pub fn begin_execution(conn: &Connection, zone_id: &str) -> rusqlite::Result<String> {
    let id = Uuid::new_v4().to_string();
    let ts = now_ts();
    conn.execute(
        "INSERT INTO organizer_executions \
         (id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json, \
          hash, prev_hash, finished) \
         VALUES (?1, ?2, ?3, 0, 0, 0, '[]', '[]', '', '', 0)",
        params![id, zone_id, ts],
    )?;
    Ok(id)
}

/// Durably overwrite the report-so-far for an execution begun with
/// [`begin_execution`]. Called after every action
/// (`organizer::executor::execute_plan_with_progress`), so a crash between
/// two actions loses at most the most recent snapshot write — never the
/// whole run's undo journal. Still unsealed; sealing happens once, in
/// [`finish_execution`].
pub fn update_execution_progress(
    conn: &Connection,
    id: &str,
    report: &ExecutionReport,
) -> rusqlite::Result<()> {
    let audit_json = serde_json::to_string(&report.audit).map_err(to_sqlite_err)?;
    let undo_json = serde_json::to_string(&report.undo).map_err(to_sqlite_err)?;
    conn.execute(
        "UPDATE organizer_executions \
         SET applied = ?2, skipped = ?3, failed = ?4, audit_json = ?5, undo_json = ?6 \
         WHERE id = ?1",
        params![
            id,
            report.applied as i64,
            report.skipped as i64,
            report.failed as i64,
            audit_json,
            undo_json,
        ],
    )?;
    Ok(())
}

/// Finalize a run started with [`begin_execution`]: write its final content,
/// seal it into the tamper-evident chain (same scheme as [`save_execution`]),
/// and mark it finished. Excludes this row's own id when locating the chain
/// tip, since `begin_execution` already inserted it (with an empty seal)
/// before this runs — without the exclusion, a run would (harmlessly but
/// incorrectly) chain to itself instead of the run before it.
pub fn finish_execution(
    conn: &Connection,
    id: &str,
    zone_id: &str,
    report: &ExecutionReport,
) -> rusqlite::Result<()> {
    let ts: String = conn.query_row(
        "SELECT created_at FROM organizer_executions WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    let audit_json = serde_json::to_string(&report.audit).map_err(to_sqlite_err)?;
    let undo_json = serde_json::to_string(&report.undo).map_err(to_sqlite_err)?;

    let prev_hash: String = conn
        .query_row(
            "SELECT hash FROM organizer_executions WHERE id != ?1 ORDER BY rowid DESC LIMIT 1",
            params![id],
            |row| row.get(0),
        )
        .unwrap_or_default();
    let hash = execution_row_hash(
        &prev_hash,
        id,
        zone_id,
        &ts,
        report.applied,
        report.skipped,
        report.failed,
        &audit_json,
        &undo_json,
    );

    conn.execute(
        "UPDATE organizer_executions \
         SET applied = ?2, skipped = ?3, failed = ?4, audit_json = ?5, undo_json = ?6, \
             hash = ?7, prev_hash = ?8, finished = 1 \
         WHERE id = ?1",
        params![
            id,
            report.applied as i64,
            report.skipped as i64,
            report.failed as i64,
            audit_json,
            undo_json,
            hash,
            prev_hash,
        ],
    )?;
    Ok(())
}

/// Mark an execution finished without changing its content — for a run the
/// user has resolved (undone, or explicitly dismissed) without going through
/// `finish_execution`'s sealing (an interrupted run's content was already
/// durably written by the last `update_execution_progress` before the
/// crash; there is nothing further to seal).
pub fn mark_execution_finished(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM organizer_executions WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    conn.execute(
        "UPDATE organizer_executions SET finished = 1 WHERE id = ?1",
        params![id],
    )?;
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
pub fn find_unfinished_execution(conn: &Connection) -> rusqlite::Result<Option<StoredExecution>> {
    let mut stmt = conn.prepare(
        "SELECT id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json, \
         hash, prev_hash, finished \
         FROM organizer_executions WHERE finished = 0 ORDER BY rowid DESC LIMIT 1",
    )?;
    let mut rows = stmt.query_map([], |row| {
        let audit_json: String = row.get(6)?;
        let undo_json: String = row.get(7)?;
        let audit = serde_json::from_str(&audit_json).map_err(to_sqlite_err)?;
        let undo = serde_json::from_str(&undo_json).map_err(to_sqlite_err)?;
        Ok(StoredExecution {
            id: row.get(0)?,
            zone_id: row.get(1)?,
            created_at: row.get(2)?,
            applied: row.get::<_, i64>(3)? as usize,
            skipped: row.get::<_, i64>(4)? as usize,
            failed: row.get::<_, i64>(5)? as usize,
            audit,
            undo,
            hash: row.get(8)?,
            prev_hash: row.get(9)?,
            finished: row.get::<_, i64>(10)? != 0,
        })
    })?;
    match rows.next() {
        Some(execution) => Ok(Some(execution?)),
        None => Ok(None),
    }
}

/// Delete executions that fall outside the retention policy, returning how many
/// rows were removed. Both bounds are optional; when both are `None` this is a
/// no-op (the default — Ghost keeps all audit history unless the user opts in).
///
/// `keep_last` retains the most recent N runs (by rowid, i.e. insertion order);
/// `keep_days` deletes runs whose `created_at` is older than `now - days`. When
/// both are set, a run is deleted if *either* bound would drop it. Because both
/// bounds only ever remove an oldest prefix, the retained runs stay a contiguous
/// suffix and their hash chain remains internally verifiable.
pub fn prune_executions(
    conn: &Connection,
    keep_last: Option<usize>,
    keep_days: Option<u64>,
) -> rusqlite::Result<usize> {
    let mut deleted = 0usize;

    if let Some(keep) = keep_last {
        // Delete everything except the `keep` highest rowids.
        deleted += conn.execute(
            "DELETE FROM organizer_executions WHERE rowid NOT IN \
             (SELECT rowid FROM organizer_executions ORDER BY rowid DESC LIMIT ?1)",
            params![keep as i64],
        )?;
    }

    if let Some(days) = keep_days {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // created_at is epoch-seconds stored as text; compare numerically.
        let cutoff = now.saturating_sub(days.saturating_mul(86_400));
        deleted += conn.execute(
            "DELETE FROM organizer_executions \
             WHERE CAST(created_at AS INTEGER) < ?1",
            params![cutoff as i64],
        )?;
    }

    Ok(deleted)
}

/// Verify the execution hash chain end to end. Walks runs in insertion order
/// (by rowid) and, for each *sealed* run (one written since the V5 migration),
/// recomputes its hash from its stored bytes and checks that (a) it matches the
/// recorded `hash` and (b) its `prev_hash` matches the previous sealed run's
/// hash. The first sealed run is the anchor — its `prev_hash` may point at a
/// pruned or genesis entry, which is expected, not a break. Pre-V5 rows carry
/// no seal and are counted separately rather than failing the check.
pub fn verify_chain(conn: &Connection) -> rusqlite::Result<ChainVerification> {
    let mut stmt = conn.prepare(
        "SELECT id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json, \
         hash, prev_hash \
         FROM organizer_executions ORDER BY rowid ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(VerifyRow {
            id: row.get(0)?,
            zone_id: row.get(1)?,
            created_at: row.get(2)?,
            applied: row.get::<_, i64>(3)? as usize,
            skipped: row.get::<_, i64>(4)? as usize,
            failed: row.get::<_, i64>(5)? as usize,
            audit_json: row.get(6)?,
            undo_json: row.get(7)?,
            hash: row.get(8)?,
            prev_hash: row.get(9)?,
        })
    })?;

    let mut sealed_count = 0usize;
    let mut unsealed_count = 0usize;
    let mut first_break: Option<ChainBreak> = None;
    let mut expected_prev: Option<String> = None; // hash of the previous sealed run

    for row in rows {
        let row = row?;
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

/// Internal row shape for chain verification.
struct VerifyRow {
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
}

/// List past executions, newest first.
pub fn list_executions(conn: &Connection) -> rusqlite::Result<Vec<ExecutionSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, zone_id, created_at, applied, skipped, failed, hash, finished \
         FROM organizer_executions ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let hash: String = row.get(6)?;
        Ok(ExecutionSummary {
            id: row.get(0)?,
            zone_id: row.get(1)?,
            created_at: row.get(2)?,
            applied: row.get::<_, i64>(3)? as usize,
            skipped: row.get::<_, i64>(4)? as usize,
            failed: row.get::<_, i64>(5)? as usize,
            sealed: !hash.is_empty(),
            finished: row.get::<_, i64>(7)? != 0,
        })
    })?;
    rows.collect()
}

/// Load a single past execution with its audit log and undo journal.
pub fn get_execution(conn: &Connection, id: &str) -> rusqlite::Result<Option<StoredExecution>> {
    let mut stmt = conn.prepare(
        "SELECT id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json, \
         hash, prev_hash, finished \
         FROM organizer_executions WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        let audit_json: String = row.get(6)?;
        let undo_json: String = row.get(7)?;
        let audit = serde_json::from_str(&audit_json).map_err(to_sqlite_err)?;
        let undo = serde_json::from_str(&undo_json).map_err(to_sqlite_err)?;
        Ok(StoredExecution {
            id: row.get(0)?,
            zone_id: row.get(1)?,
            created_at: row.get(2)?,
            applied: row.get::<_, i64>(3)? as usize,
            skipped: row.get::<_, i64>(4)? as usize,
            failed: row.get::<_, i64>(5)? as usize,
            audit,
            undo,
            hash: row.get(8)?,
            prev_hash: row.get(9)?,
            finished: row.get::<_, i64>(10)? != 0,
        })
    })?;
    match rows.next() {
        Some(execution) => Ok(Some(execution?)),
        None => Ok(None),
    }
}

/// Map a JSON (de)serialization error into a rusqlite error so it threads
/// through `query_map` closures and the `?` operator cleanly.
fn to_sqlite_err(e: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
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

    #[test]
    fn save_and_load_round_trips_audit_and_undo() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        tmp.file("song.mp3", b"b");
        let rules = vec![full_rule(tmp.path())];

        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);
        assert!(report.applied >= 2);

        let conn = open_in_memory().unwrap();
        let id = save_execution(&conn, "z", &report).unwrap();

        // Summary list reflects the run.
        let list = list_executions(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(list[0].applied, report.applied);

        // Full load reconstructs the audit log and undo journal byte-for-byte.
        let loaded = get_execution(&conn, &id).unwrap().unwrap();
        assert_eq!(loaded.audit, report.audit);
        assert_eq!(loaded.undo, report.undo);
        assert_eq!(loaded.zone_id, "z");

        // save_execution is the one-shot legacy path: rows it writes are
        // already finished, never surfaced as needing recovery.
        assert!(loaded.finished);
        assert!(list[0].finished);
        assert!(find_unfinished_execution(&conn).unwrap().is_none());
    }

    #[test]
    fn begin_execution_writes_an_unfinished_unsealed_row() {
        let conn = open_in_memory().unwrap();
        let id = begin_execution(&conn, "z").unwrap();

        let loaded = get_execution(&conn, &id).unwrap().unwrap();
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

        let conn = open_in_memory().unwrap();
        let id = begin_execution(&conn, "z").unwrap();

        // Simulate the executor's progress callback firing partway through a
        // run, then the process dying before finish_execution ever runs.
        let mut last_report = None;
        execute_plan_with_progress(&plan, &rules, |report| {
            update_execution_progress(&conn, &id, report).unwrap();
            last_report = Some(report.clone());
        });
        let last_report = last_report.expect("plan had at least one action");
        // (no finish_execution call — this is the "crash" point)

        let recovered = find_unfinished_execution(&conn)
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

        let conn = open_in_memory().unwrap();
        // A prior, already-sealed run establishes a chain tip to link to.
        let earlier = execute_plan(&plan_with_rules("z", &rules), &rules);
        let _ = save_execution(&conn, "z", &earlier).unwrap();
        let expected_prev_hash = get_execution(&conn, &list_executions(&conn).unwrap()[0].id)
            .unwrap()
            .unwrap()
            .hash;

        let id = begin_execution(&conn, "z").unwrap();
        let report = execute_plan(&plan, &rules);
        finish_execution(&conn, &id, "z", &report).unwrap();

        let loaded = get_execution(&conn, &id).unwrap().unwrap();
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
        assert!(find_unfinished_execution(&conn).unwrap().is_none());

        // The chain (including the WAL-style run) still verifies end to end.
        assert!(verify_chain(&conn).unwrap().intact);
    }

    #[test]
    fn mark_execution_finished_resolves_without_resealing() {
        let conn = open_in_memory().unwrap();
        let id = begin_execution(&conn, "z").unwrap();
        assert!(find_unfinished_execution(&conn).unwrap().is_some());

        mark_execution_finished(&conn, &id).unwrap();

        assert!(find_unfinished_execution(&conn).unwrap().is_none());
        let loaded = get_execution(&conn, &id).unwrap().unwrap();
        assert!(loaded.finished);
        assert_eq!(
            loaded.hash, "",
            "resolving without finish_execution stays unsealed"
        );
    }

    #[test]
    fn get_execution_returns_none_for_unknown_id() {
        let conn = open_in_memory().unwrap();
        assert!(get_execution(&conn, "missing").unwrap().is_none());
    }

    #[test]
    fn list_executions_is_empty_on_a_fresh_database() {
        let conn = open_in_memory().unwrap();
        assert!(list_executions(&conn).unwrap().is_empty());
    }

    /// A saved run carries a non-empty seal and is reported as sealed.
    #[test]
    fn a_saved_run_is_sealed() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);

        let conn = open_in_memory().unwrap();
        let id = save_execution(&conn, "z", &report).unwrap();
        let loaded = get_execution(&conn, &id).unwrap().unwrap();
        assert!(!loaded.hash.is_empty(), "a saved run must be sealed");
        assert_eq!(loaded.prev_hash, "", "the first run is the genesis");
        assert!(list_executions(&conn).unwrap()[0].sealed);
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

        let conn = open_in_memory().unwrap();
        // Saving the same report several times still produces distinct, chained
        // rows (fresh id per save); the chain mechanics are what we're testing.
        for _ in 0..3 {
            save_execution(&conn, "z", &report).unwrap();
        }

        let v = verify_chain(&conn).unwrap();
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

        let conn = open_in_memory().unwrap();
        let first = save_execution(&conn, "z", &report).unwrap();
        save_execution(&conn, "z", &report).unwrap();

        // Alter the stored audit of the first run — the seal no longer matches.
        conn.execute(
            "UPDATE organizer_executions SET audit_json = '[]' WHERE id = ?1",
            params![first],
        )
        .unwrap();

        let v = verify_chain(&conn).unwrap();
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

        let conn = open_in_memory().unwrap();
        for _ in 0..3 {
            save_execution(&conn, "z", &report).unwrap();
        }

        let deleted = prune_executions(&conn, Some(1), None).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(list_executions(&conn).unwrap().len(), 1);
        // The retained suffix still verifies among itself.
        assert!(verify_chain(&conn).unwrap().intact);
    }

    #[test]
    fn prune_keep_days_deletes_old_runs_only() {
        let conn = open_in_memory().unwrap();
        // An ancient unsealed row (created ~100 days ago) and a fresh sealed one.
        let old_ts = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 100 * 86_400)
            .to_string();
        conn.execute(
            "INSERT INTO organizer_executions \
             (id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json) \
             VALUES ('old', 'z', ?1, 0, 0, 0, '[]', '[]')",
            params![old_ts],
        )
        .unwrap();

        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);
        let fresh = save_execution(&conn, "z", &report).unwrap();

        let deleted = prune_executions(&conn, None, Some(30)).unwrap();
        assert_eq!(deleted, 1);
        assert!(get_execution(&conn, "old").unwrap().is_none());
        assert!(get_execution(&conn, &fresh).unwrap().is_some());
    }

    #[test]
    fn prune_is_a_noop_when_no_policy_is_set() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);

        let conn = open_in_memory().unwrap();
        save_execution(&conn, "z", &report).unwrap();
        save_execution(&conn, "z", &report).unwrap();

        let deleted = prune_executions(&conn, None, None).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(list_executions(&conn).unwrap().len(), 2);
    }

    /// Pre-V5 rows have no seal; they are counted as unsealed, and the sealed
    /// runs written after still verify.
    #[test]
    fn unsealed_legacy_rows_do_not_fail_verification() {
        let conn = open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO organizer_executions \
             (id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json) \
             VALUES ('legacy', 'z', '1', 0, 0, 0, '[]', '[]')",
            [],
        )
        .unwrap();

        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        let rules = vec![full_rule(tmp.path())];
        let report = execute_plan(&plan_with_rules("z", &rules), &rules);
        save_execution(&conn, "z", &report).unwrap();

        let v = verify_chain(&conn).unwrap();
        assert!(v.intact);
        assert_eq!(v.unsealed_count, 1);
        assert_eq!(v.sealed_count, 1);
    }
}
