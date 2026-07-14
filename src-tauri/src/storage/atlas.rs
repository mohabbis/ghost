//! redb-backed persistence for the Ghost Atlas semantic-memory graph.
//!
//! This is the durable half of the Atlas: [`crate::core::atlas`] holds the pure,
//! deterministic engine (embeddings, similarity, auto-linking, archival
//! selection), and this module stores nodes and edges in redb and drives that
//! engine over the persisted corpus. It follows the same trust model as the
//! rest of `storage/`: **local-only, no network, no ML runtime**, and it never
//! deletes a memory — "forgetting" flips the `archived` flag ([`archive_memory`]),
//! which [`unarchive_memory`] reverses.
//!
//! Layout:
//! - `atlas_memories`: `id -> JSON(Memory)`.
//! - `atlas_links`: `"<a>\u{1f}<b>" -> JSON(MemoryLink)`, one row per undirected
//!   pair (canonical sorted key from [`MemoryLink::key`]).
//!
//! Scale assumption: this is a personal memory store (hundreds to low thousands
//! of notes). Search and auto-link load the live corpus and embed each memory's
//! content on the fly. That is deterministic and keeps storage lean (no stored
//! vectors to migrate), and stays well within interactive latency at that scale.
//! If the corpus ever grows large enough to matter, the place to add an index is
//! here, without changing the engine.

use crate::core::atlas::{
    self, DecayOptions, LinkOptions, Memory, MemoryLink, MemorySource, SearchOptions,
};
use crate::storage::Db;
use redb::{ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MEMORIES: TableDefinition<&str, &str> = TableDefinition::new("atlas_memories");
const LINKS: TableDefinition<&str, &str> = TableDefinition::new("atlas_links");

/// Ensure both tables exist. Called once from `storage::open_default`/
/// `open_in_memory` right after the database is created.
pub(crate) fn init(write_txn: &WriteTransaction) -> anyhow::Result<()> {
    write_txn.open_table(MEMORIES)?;
    write_txn.open_table(LINKS)?;
    Ok(())
}

/// Current epoch seconds.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// A memory with its automatically discovered links, returned by [`add_memory`].
#[derive(Debug, Clone)]
pub struct AddedMemory {
    /// The stored memory.
    pub memory: Memory,
    /// Links formed to existing memories at insert time.
    pub links: Vec<MemoryLink>,
}

/// Read every stored memory. Order is redb's byte-lexicographic key order (by id);
/// callers that care about ranking use [`search`], which sorts by score.
pub fn list_memories(db: &Db, include_archived: bool) -> anyhow::Result<Vec<Memory>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(MEMORIES)?;
    let mut out = Vec::new();
    for entry in table.iter()? {
        let (_k, v) = entry?;
        let mem: Memory = serde_json::from_str(v.value())?;
        if mem.archived && !include_archived {
            continue;
        }
        out.push(mem);
    }
    Ok(out)
}

/// Fetch one memory by id.
pub fn get_memory(db: &Db, id: &str) -> anyhow::Result<Option<Memory>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(MEMORIES)?;
    match table.get(id)? {
        Some(v) => Ok(Some(serde_json::from_str(v.value())?)),
        None => Ok(None),
    }
}

/// Persist a memory record (insert or overwrite by id) inside an open write txn.
fn put_memory(write_txn: &WriteTransaction, mem: &Memory) -> anyhow::Result<()> {
    let json = serde_json::to_string(mem)?;
    let mut table = write_txn.open_table(MEMORIES)?;
    table.insert(mem.id.as_str(), json.as_str())?;
    Ok(())
}

/// Persist a link (insert or overwrite by canonical pair key) inside a write txn.
fn put_link(write_txn: &WriteTransaction, link: &MemoryLink) -> anyhow::Result<()> {
    let json = serde_json::to_string(link)?;
    let mut table = write_txn.open_table(LINKS)?;
    table.insert(link.key().as_str(), json.as_str())?;
    Ok(())
}

/// Add a new memory, auto-linking it to related existing memories.
///
/// The engine picks links deterministically ([`atlas::auto_links`]); this
/// function assigns the id/timestamps and writes the node plus its links in a
/// single write transaction. Archived memories are never linked.
pub fn add_memory(
    db: &Db,
    content: &str,
    tags: Vec<String>,
    source: MemorySource,
) -> anyhow::Result<AddedMemory> {
    let ts = now();
    let existing = list_memories(db, false)?;
    let link_hits = atlas::auto_links(content, &existing, &LinkOptions::default());

    let mem = Memory::new(Uuid::new_v4().to_string(), content, tags, source, ts);
    let links: Vec<MemoryLink> = link_hits
        .iter()
        .map(|(idx, weight)| MemoryLink::new(&mem.id, &existing[*idx].id, *weight, ts))
        .collect();

    let write_txn = db.db.begin_write()?;
    {
        put_memory(&write_txn, &mem)?;
        for link in &links {
            put_link(&write_txn, link)?;
        }
    }
    write_txn.commit()?;

    Ok(AddedMemory { memory: mem, links })
}

/// A search result carrying the memory and its similarity score.
#[derive(Debug, Clone)]
pub struct ScoredMemory {
    /// The matched memory.
    pub memory: Memory,
    /// Cosine similarity to the query.
    pub score: f32,
}

/// Rank stored memories against `query`. Read-only: it does not update access
/// bookkeeping — call [`record_access`] with the ids you actually surface, so
/// "was used" reflects real user-facing recall rather than every background scan.
pub fn search(db: &Db, query: &str, opts: &SearchOptions) -> anyhow::Result<Vec<ScoredMemory>> {
    let memories = list_memories(db, opts.include_archived)?;
    let hits = atlas::search(&memories, query, opts);
    Ok(hits
        .into_iter()
        .map(|h| ScoredMemory {
            memory: memories[h.index].clone(),
            score: h.score,
        })
        .collect())
}

/// The live memories directly linked to `id`, each with the link weight,
/// strongest first. Used to walk the graph out from a starting note.
///
/// Archived ("soft-forgotten") neighbors are omitted, mirroring [`search`]'s
/// default: the edge itself is preserved in storage (nothing is deleted), but a
/// memory the user has forgotten is not resurfaced by walking the graph.
/// [`unarchive_memory`] restores it to the walk.
pub fn neighbors(db: &Db, id: &str) -> anyhow::Result<Vec<(Memory, f32)>> {
    let read_txn = db.db.begin_read()?;
    let links_table = read_txn.open_table(LINKS)?;
    let mem_table = read_txn.open_table(MEMORIES)?;

    let mut out = Vec::new();
    for entry in links_table.iter()? {
        let (_k, v) = entry?;
        let link: MemoryLink = serde_json::from_str(v.value())?;
        if let Some(other) = link.other(id) {
            if let Some(m) = mem_table.get(other)? {
                let mem: Memory = serde_json::from_str(m.value())?;
                if mem.archived {
                    continue;
                }
                out.push((mem, link.weight));
            }
        }
    }
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.id.cmp(&b.0.id))
    });
    Ok(out)
}

/// Mark memories as accessed: bump `access_count` and set `last_accessed` to now.
/// Missing ids are skipped. Returns how many rows were updated.
pub fn record_access(db: &Db, ids: &[String]) -> anyhow::Result<usize> {
    let ts = now();
    let write_txn = db.db.begin_write()?;
    let mut updated = 0;
    {
        // Read-modify-write each memory; single-writer redb makes this race-free.
        let existing: Vec<Option<Memory>> = {
            let table = write_txn.open_table(MEMORIES)?;
            ids.iter()
                .map(|id| {
                    table
                        .get(id.as_str())
                        .ok()
                        .flatten()
                        .and_then(|v| serde_json::from_str::<Memory>(v.value()).ok())
                })
                .collect()
        };
        for mem in existing.into_iter().flatten() {
            let mut mem = mem;
            mem.access_count = mem.access_count.saturating_add(1);
            mem.last_accessed = ts;
            put_memory(&write_txn, &mem)?;
            updated += 1;
        }
    }
    write_txn.commit()?;
    Ok(updated)
}

/// Soft-forget a memory: set `archived = true`. Reversible via [`unarchive_memory`];
/// the row and its links are never removed. Returns whether the memory existed.
pub fn archive_memory(db: &Db, id: &str) -> anyhow::Result<bool> {
    set_archived(db, id, true)
}

/// Restore an archived memory: set `archived = false`.
pub fn unarchive_memory(db: &Db, id: &str) -> anyhow::Result<bool> {
    set_archived(db, id, false)
}

fn set_archived(db: &Db, id: &str, archived: bool) -> anyhow::Result<bool> {
    let write_txn = db.db.begin_write()?;
    let found;
    {
        let existing = {
            let table = write_txn.open_table(MEMORIES)?;
            let found = table
                .get(id)?
                .map(|v| serde_json::from_str::<Memory>(v.value()))
                .transpose()?;
            found
        };
        match existing {
            Some(mut mem) => {
                mem.archived = archived;
                put_memory(&write_txn, &mem)?;
                found = true;
            }
            None => found = false,
        }
    }
    write_txn.commit()?;
    Ok(found)
}

/// Run one automatic-archival pass: archive every live memory that is stale under
/// `opts` ([`atlas::archival_candidates`]). Nothing is deleted. Returns the ids
/// that were archived, so the caller can audit the maintenance action.
pub fn run_archival(db: &Db, opts: &DecayOptions) -> anyhow::Result<Vec<String>> {
    let ts = now();
    let memories = list_memories(db, false)?;
    let candidates = atlas::archival_candidates(&memories, ts, opts);
    let ids: Vec<String> = candidates
        .into_iter()
        .map(|i| memories[i].id.clone())
        .collect();

    if ids.is_empty() {
        return Ok(ids);
    }

    let write_txn = db.db.begin_write()?;
    {
        for mem in memories.iter().filter(|m| ids.contains(&m.id)) {
            let mut archived = mem.clone();
            archived.archived = true;
            put_memory(&write_txn, &archived)?;
        }
    }
    write_txn.commit()?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory;

    #[test]
    fn add_and_get_round_trips() {
        let db = open_in_memory().unwrap();
        let added = add_memory(
            &db,
            "quarterly finance report for Q3",
            vec!["finance".into()],
            MemorySource::Manual,
        )
        .unwrap();
        assert!(added.links.is_empty(), "first memory has nothing to link");

        let fetched = get_memory(&db, &added.memory.id).unwrap().unwrap();
        assert_eq!(fetched.content, "quarterly finance report for Q3");
        assert_eq!(fetched.tags, vec!["finance".to_string()]);
        assert_eq!(fetched.access_count, 0);
        assert!(!fetched.archived);
    }

    #[test]
    fn adding_related_memory_auto_links() {
        let db = open_in_memory().unwrap();
        let first = add_memory(
            &db,
            "ghost organizer release plan",
            vec![],
            MemorySource::Manual,
        )
        .unwrap();
        let second = add_memory(
            &db,
            "ghost organizer release checklist",
            vec![],
            MemorySource::Manual,
        )
        .unwrap();

        assert_eq!(
            second.links.len(),
            1,
            "should link to the related first note"
        );
        let neigh = neighbors(&db, &first.memory.id).unwrap();
        assert_eq!(neigh.len(), 1);
        assert_eq!(neigh[0].0.id, second.memory.id);
        assert!(neigh[0].1 > 0.0);
    }

    #[test]
    fn neighbors_excludes_archived_and_restores_on_unarchive() {
        let db = open_in_memory().unwrap();
        let a = add_memory(
            &db,
            "ghost organizer release plan",
            vec![],
            MemorySource::Manual,
        )
        .unwrap();
        let b = add_memory(
            &db,
            "ghost organizer release checklist",
            vec![],
            MemorySource::Manual,
        )
        .unwrap();
        // The two related notes auto-link, so each is the other's neighbor.
        assert_eq!(b.links.len(), 1, "related notes should link");
        assert_eq!(neighbors(&db, &a.memory.id).unwrap().len(), 1);

        // Forgetting B removes it from A's neighbor walk — a soft-forgotten memory
        // must not be resurfaced through the graph, matching search's default.
        assert!(archive_memory(&db, &b.memory.id).unwrap());
        assert!(
            neighbors(&db, &a.memory.id).unwrap().is_empty(),
            "archived neighbor must not surface in the walk"
        );

        // The edge was never deleted: unarchiving restores B to the walk.
        assert!(unarchive_memory(&db, &b.memory.id).unwrap());
        assert_eq!(
            neighbors(&db, &a.memory.id).unwrap().len(),
            1,
            "unarchiving must restore the neighbor"
        );
    }

    #[test]
    fn search_finds_by_meaning_and_excludes_archived() {
        let db = open_in_memory().unwrap();
        add_memory(
            &db,
            "renew the TLS certificate before it expires",
            vec![],
            MemorySource::Manual,
        )
        .unwrap();
        let groceries = add_memory(&db, "buy milk and eggs", vec![], MemorySource::Manual).unwrap();

        let hits = search(&db, "certificate renewal", &SearchOptions::default()).unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].memory.content.contains("certificate"));

        // Archiving the top hit removes it from default search.
        let top_id = hits[0].memory.id.clone();
        assert!(archive_memory(&db, &top_id).unwrap());
        let after = search(&db, "certificate renewal", &SearchOptions::default()).unwrap();
        assert!(after.iter().all(|h| h.memory.id != top_id));

        // The unrelated note is still there and unaffected.
        assert!(get_memory(&db, &groceries.memory.id).unwrap().is_some());
    }

    #[test]
    fn focus_tag_scopes_search() {
        let db = open_in_memory().unwrap();
        add_memory(
            &db,
            "write the release notes",
            vec!["ghost".into()],
            MemorySource::Manual,
        )
        .unwrap();
        add_memory(
            &db,
            "write the release notes",
            vec!["other".into()],
            MemorySource::Manual,
        )
        .unwrap();

        let hits = search(
            &db,
            "release notes",
            &SearchOptions {
                focus_tag: Some("ghost".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].memory.has_tag("ghost"));
    }

    #[test]
    fn record_access_bumps_count_and_time() {
        let db = open_in_memory().unwrap();
        let m = add_memory(&db, "a note to recall", vec![], MemorySource::Manual).unwrap();
        assert_eq!(m.memory.access_count, 0);

        let updated = record_access(&db, std::slice::from_ref(&m.memory.id)).unwrap();
        assert_eq!(updated, 1);

        let after = get_memory(&db, &m.memory.id).unwrap().unwrap();
        assert_eq!(after.access_count, 1);
        assert!(after.last_accessed >= m.memory.created_at);

        // Unknown ids are skipped, not errors.
        assert_eq!(record_access(&db, &["nope".into()]).unwrap(), 0);
    }

    #[test]
    fn archive_and_unarchive_are_reversible() {
        let db = open_in_memory().unwrap();
        let m = add_memory(&db, "reversible note", vec![], MemorySource::Manual).unwrap();

        assert!(archive_memory(&db, &m.memory.id).unwrap());
        assert!(get_memory(&db, &m.memory.id).unwrap().unwrap().archived);

        assert!(unarchive_memory(&db, &m.memory.id).unwrap());
        assert!(!get_memory(&db, &m.memory.id).unwrap().unwrap().archived);

        // Missing id reports not-found rather than erroring.
        assert!(!archive_memory(&db, "missing").unwrap());
    }

    #[test]
    fn run_archival_soft_forgets_only_stale_memories() {
        let db = open_in_memory().unwrap();
        let stale = add_memory(&db, "old stale note", vec![], MemorySource::Manual).unwrap();
        let fresh = add_memory(
            &db,
            "a totally different recent note",
            vec![],
            MemorySource::Manual,
        )
        .unwrap();

        // Backdate the first memory well past the idle window.
        {
            let mut m = get_memory(&db, &stale.memory.id).unwrap().unwrap();
            m.last_accessed = 1; // epoch second 1: ancient
            let write_txn = db.db.begin_write().unwrap();
            put_memory(&write_txn, &m).unwrap();
            write_txn.commit().unwrap();
        }

        let archived = run_archival(&db, &DecayOptions::default()).unwrap();
        assert_eq!(archived, vec![stale.memory.id.clone()]);
        assert!(get_memory(&db, &stale.memory.id).unwrap().unwrap().archived);
        assert!(!get_memory(&db, &fresh.memory.id).unwrap().unwrap().archived);
    }
}
