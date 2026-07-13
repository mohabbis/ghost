//! Write-ahead durable storage for routine replay runs.
//!
//! Mirrors the Organizer execution WAL (`storage::executions`) at a smaller
//! scope: begin before replay, update after every event, finish on normal
//! completion. Interrupted runs stay `finished = false` for recovery.

use crate::audit::replay_undo_journal::{ReplayRunReport, ReplayUndoJournal};
use crate::storage::Db;
use redb::{ReadTransaction, ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const REPLAY_RUNS: TableDefinition<u64, &[u8]> = TableDefinition::new("replay_runs");
const REPLAY_RUNS_BY_ID: TableDefinition<&str, u64> = TableDefinition::new("replay_runs_by_id");

pub(crate) fn init(write_txn: &WriteTransaction) -> anyhow::Result<()> {
    write_txn.open_table(REPLAY_RUNS)?;
    write_txn.open_table(REPLAY_RUNS_BY_ID)?;
    Ok(())
}

fn now_ts() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayRunSummary {
    pub id: String,
    pub workflow_name: String,
    pub workflow_fingerprint: String,
    pub created_at: String,
    pub events_applied: usize,
    pub events_total: usize,
    pub reversible_backspaces: u32,
    #[serde(default = "default_true")]
    pub finished: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredReplayRun {
    pub id: String,
    pub workflow_name: String,
    pub workflow_fingerprint: String,
    pub created_at: String,
    pub events_applied: usize,
    pub events_total: usize,
    pub undo: ReplayUndoJournal,
    pub finished: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReplayRunRow {
    id: String,
    workflow_name: String,
    workflow_fingerprint: String,
    created_at: String,
    events_applied: usize,
    events_total: usize,
    undo_json: String,
    finished: bool,
}

fn insert_row(write_txn: &WriteTransaction, row: &ReplayRunRow) -> anyhow::Result<u64> {
    let mut table = write_txn.open_table(REPLAY_RUNS)?;
    let mut by_id = write_txn.open_table(REPLAY_RUNS_BY_ID)?;
    let seq = table.last()?.map(|(k, _)| k.value() + 1).unwrap_or(1);
    let bytes = serde_json::to_vec(row)?;
    table.insert(seq, bytes.as_slice())?;
    by_id.insert(row.id.as_str(), seq)?;
    Ok(seq)
}

fn seq_for_id_write(write_txn: &WriteTransaction, id: &str) -> anyhow::Result<u64> {
    let by_id = write_txn.open_table(REPLAY_RUNS_BY_ID)?;
    let seq = by_id
        .get(id)?
        .map(|g| g.value())
        .ok_or_else(|| anyhow::anyhow!("unknown replay run id"))?;
    Ok(seq)
}

fn seq_for_id_read(read_txn: &ReadTransaction, id: &str) -> anyhow::Result<u64> {
    let by_id = read_txn.open_table(REPLAY_RUNS_BY_ID)?;
    let seq = by_id
        .get(id)?
        .map(|g| g.value())
        .ok_or_else(|| anyhow::anyhow!("unknown replay run id"))?;
    Ok(seq)
}

fn read_row_write(write_txn: &WriteTransaction, seq: u64) -> anyhow::Result<ReplayRunRow> {
    let table = write_txn.open_table(REPLAY_RUNS)?;
    let bytes = table
        .get(seq)?
        .ok_or_else(|| anyhow::anyhow!("missing replay run"))?;
    Ok(serde_json::from_slice(bytes.value())?)
}

fn read_row_read(read_txn: &ReadTransaction, seq: u64) -> anyhow::Result<ReplayRunRow> {
    let table = read_txn.open_table(REPLAY_RUNS)?;
    let bytes = table
        .get(seq)?
        .ok_or_else(|| anyhow::anyhow!("missing replay run"))?;
    Ok(serde_json::from_slice(bytes.value())?)
}

fn update_row<F>(write_txn: &WriteTransaction, id: &str, mut f: F) -> anyhow::Result<()>
where
    F: FnMut(&mut ReplayRunRow),
{
    let seq = seq_for_id_write(write_txn, id)?;
    let mut row = read_row_write(write_txn, seq)?;
    f(&mut row);
    let bytes = serde_json::to_vec(&row)?;
    write_txn
        .open_table(REPLAY_RUNS)?
        .insert(seq, bytes.as_slice())?;
    Ok(())
}

fn row_to_stored(row: ReplayRunRow) -> anyhow::Result<StoredReplayRun> {
    Ok(StoredReplayRun {
        id: row.id,
        workflow_name: row.workflow_name,
        workflow_fingerprint: row.workflow_fingerprint,
        created_at: row.created_at,
        events_applied: row.events_applied,
        events_total: row.events_total,
        undo: serde_json::from_str(&row.undo_json)?,
        finished: row.finished,
    })
}

fn row_to_summary(row: &ReplayRunRow) -> ReplayRunSummary {
    let undo: ReplayUndoJournal = serde_json::from_str(&row.undo_json).unwrap_or_default();
    ReplayRunSummary {
        id: row.id.clone(),
        workflow_name: row.workflow_name.clone(),
        workflow_fingerprint: row.workflow_fingerprint.clone(),
        created_at: row.created_at.clone(),
        events_applied: row.events_applied,
        events_total: row.events_total,
        reversible_backspaces: undo.reversible_backspace_count(),
        finished: row.finished,
    }
}

pub fn begin_replay_run(
    db: &Db,
    workflow_name: &str,
    workflow_fingerprint: &str,
    events_total: usize,
) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    let row = ReplayRunRow {
        id: id.clone(),
        workflow_name: workflow_name.to_string(),
        workflow_fingerprint: workflow_fingerprint.to_string(),
        created_at: now_ts(),
        events_applied: 0,
        events_total,
        undo_json: "[]".to_string(),
        finished: false,
    };
    let write_txn = db.db.begin_write()?;
    insert_row(&write_txn, &row)?;
    write_txn.commit()?;
    Ok(id)
}

pub fn update_replay_progress(db: &Db, id: &str, report: &ReplayRunReport) -> anyhow::Result<()> {
    let undo_json = serde_json::to_string(&report.undo)?;
    let write_txn = db.db.begin_write()?;
    update_row(&write_txn, id, |row| {
        row.events_applied = report.events_applied;
        row.events_total = report.events_total;
        row.undo_json = undo_json.clone();
    })?;
    write_txn.commit()?;
    Ok(())
}

pub fn finish_replay_run(db: &Db, id: &str, report: &ReplayRunReport) -> anyhow::Result<()> {
    let undo_json = serde_json::to_string(&report.undo)?;
    let write_txn = db.db.begin_write()?;
    update_row(&write_txn, id, |row| {
        row.events_applied = report.events_applied;
        row.events_total = report.events_total;
        row.undo_json = undo_json.clone();
        row.finished = true;
    })?;
    write_txn.commit()?;
    Ok(())
}

pub fn mark_replay_run_finished(db: &Db, id: &str) -> anyhow::Result<()> {
    let write_txn = db.db.begin_write()?;
    update_row(&write_txn, id, |row| row.finished = true)?;
    write_txn.commit()?;
    Ok(())
}

pub fn get_replay_run(db: &Db, id: &str) -> anyhow::Result<StoredReplayRun> {
    let read_txn = db.db.begin_read()?;
    let seq = seq_for_id_read(&read_txn, id)?;
    row_to_stored(read_row_read(&read_txn, seq)?)
}

fn find_latest_unfinished_row(txn: &ReadTransaction) -> anyhow::Result<Option<ReplayRunRow>> {
    let table = txn.open_table(REPLAY_RUNS)?;
    let mut best: Option<(u64, ReplayRunRow)> = None;
    for entry in table.iter()? {
        let (k, bytes) = entry?;
        let row: ReplayRunRow = serde_json::from_slice(bytes.value())?;
        if !row.finished {
            let seq = k.value();
            if best.as_ref().is_none_or(|(s, _)| seq > *s) {
                best = Some((seq, row));
            }
        }
    }
    Ok(best.map(|(_, row)| row))
}

pub fn find_unfinished_replay_run(db: &Db) -> anyhow::Result<Option<StoredReplayRun>> {
    let read_txn = db.db.begin_read()?;
    find_latest_unfinished_row(&read_txn)?
        .map(row_to_stored)
        .transpose()
}

pub fn find_unfinished_replay_summary(db: &Db) -> anyhow::Result<Option<ReplayRunSummary>> {
    let read_txn = db.db.begin_read()?;
    Ok(find_latest_unfinished_row(&read_txn)?
        .as_ref()
        .map(row_to_summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::replay_undo_journal::ReplayUndoJournal;
    use crate::storage::open_in_memory;

    fn sample_report(applied: usize, total: usize) -> ReplayRunReport {
        ReplayRunReport {
            events_applied: applied,
            events_total: total,
            undo: ReplayUndoJournal::new(),
        }
    }

    #[test]
    fn begin_and_finish_replay_run_round_trip() {
        let db = open_in_memory().unwrap();
        let id = begin_replay_run(&db, "Demo", "fp-1", 5).unwrap();
        update_replay_progress(&db, &id, &sample_report(2, 5)).unwrap();
        finish_replay_run(&db, &id, &sample_report(5, 5)).unwrap();
        assert!(find_unfinished_replay_run(&db).unwrap().is_none());
        let stored = get_replay_run(&db, &id).unwrap();
        assert!(stored.finished);
        assert_eq!(stored.events_applied, 5);
    }

    #[test]
    fn update_replay_progress_survives_a_simulated_crash() {
        let db = open_in_memory().unwrap();
        let id = begin_replay_run(&db, "Demo", "fp-2", 3).unwrap();
        update_replay_progress(&db, &id, &sample_report(1, 3)).unwrap();
        update_replay_progress(&db, &id, &sample_report(2, 3)).unwrap();
        let recovered = find_unfinished_replay_run(&db)
            .unwrap()
            .expect("unfinished run");
        assert_eq!(recovered.id, id);
        assert_eq!(recovered.events_applied, 2);
        assert!(!recovered.finished);
        mark_replay_run_finished(&db, &id).unwrap();
        assert!(find_unfinished_replay_run(&db).unwrap().is_none());
    }
}
