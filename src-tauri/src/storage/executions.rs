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
        "SELECT id, zone_id, created_at, applied, skipped, failed, hash \
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
        })
    })?;
    rows.collect()
}

/// Load a single past execution with its audit log and undo journal.
pub fn get_execution(conn: &Connection, id: &str) -> rusqlite::Result<Option<StoredExecution>> {
    let mut stmt = conn.prepare(
        "SELECT id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json, \
         hash, prev_hash \
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
    use crate::organizer::executor::execute_plan;
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
