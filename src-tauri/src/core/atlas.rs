//! Ghost Atlas — a deterministic, local semantic-memory graph.
//!
//! This is Ghost's adaptation of the "Neural Atlas" idea from other assistants
//! (a persistent memory graph that grows over time and retrieves by meaning),
//! rebuilt to fit Ghost's trust model rather than ported verbatim:
//!
//! - **Local and offline.** No network, no cloud, no model download. Embeddings
//!   are a pure, deterministic function of the text (character-trigram + word
//!   hashing, L2-normalized), so the same note always yields the same vector
//!   and retrieval is reproducible and testable without a GPU or ML runtime.
//!   This is **lexical** similarity, not neural sentence-transformer semantics —
//!   do not describe it as the latter (CLAUDE.md rule 10, no overclaiming).
//! - **No silent delete.** "Forgetting" is archival: a memory is flagged
//!   [`Memory::archived`], never removed, matching Ghost's deny-silent-delete
//!   stance. Archival is reversible.
//! - **Deterministic core, no UI, no I/O.** This module is pure logic; the
//!   embeddings are derived from content on the fly (never persisted, so they
//!   can never desync), persistence lives in `storage::atlas`, and any Tauri
//!   command surface must still go through the normal trust pipeline
//!   (module + risk class + policy + approval + audit/undo).
//!
//! See `docs/neural-atlas.md` for the full design and its boundaries.

use serde::{Deserialize, Serialize};

/// Dimensionality of the deterministic lexical embedding. Fixed so vectors are
/// comparable across memories without any stored state. 256 buckets keeps
/// hash-collision noise low for short personal notes while staying cheap to
/// compute for every memory on every search.
pub const EMBED_DIM: usize = 256;

/// Where a memory came from. Provenance is kept so a future review/audit surface
/// can distinguish a note the user typed from one a routine or the Organizer
/// proposed — Ghost never blurs "the user said" with "the app inferred."
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySource {
    /// Entered explicitly by the user.
    Manual,
    /// Captured from a recorded/replayed routine.
    Routine,
    /// Derived from an Organizer run.
    Organizer,
    /// Inferred from observed behavior (suggestion-only provenance).
    Observation,
    /// Brought in from an external export/import.
    Import,
}

/// A single memory node in the Atlas.
///
/// The embedding is intentionally **not** a field: it is derived from
/// [`Memory::content`] via [`embed`] whenever needed, so there is no stored
/// vector to migrate or desync. Edges between memories live separately (see
/// [`MemoryLink`]) so the graph can be walked without rewriting nodes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Memory {
    /// Stable unique id (assigned by the storage layer).
    pub id: String,
    /// The remembered text.
    pub content: String,
    /// Free-form tags, used for the focus filter (e.g. a project name).
    pub tags: Vec<String>,
    /// Provenance.
    pub source: MemorySource,
    /// Epoch seconds when the memory was created.
    pub created_at: u64,
    /// Epoch seconds when the memory was last surfaced to the user.
    pub last_accessed: u64,
    /// How many times the memory has been surfaced.
    pub access_count: u32,
    /// Soft-forgotten. Archived memories are excluded from search by default but
    /// never deleted.
    pub archived: bool,
}

impl Memory {
    /// Build a new memory. `id`, `created_at`, and `last_accessed` are supplied
    /// by the caller (the storage layer) so this stays pure and testable.
    pub fn new(
        id: impl Into<String>,
        content: impl Into<String>,
        tags: Vec<String>,
        source: MemorySource,
        now: u64,
    ) -> Self {
        Memory {
            id: id.into(),
            content: content.into(),
            tags,
            source,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            archived: false,
        }
    }

    /// Whether this memory carries the given focus tag (case-insensitive).
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }
}

/// An undirected, weighted link between two memories, discovered automatically
/// from lexical similarity at insert time. Stored once per pair.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MemoryLink {
    /// One endpoint id.
    pub a: String,
    /// The other endpoint id.
    pub b: String,
    /// Similarity weight in `[0, 1]` at the time the link was formed.
    pub weight: f32,
    /// Epoch seconds when the link was formed.
    pub created_at: u64,
}

impl MemoryLink {
    /// Build a link with endpoints stored in a canonical (sorted) order so a
    /// pair is represented exactly once regardless of insertion direction.
    pub fn new(x: &str, y: &str, weight: f32, now: u64) -> Self {
        let (a, b) = if x <= y { (x, y) } else { (y, x) };
        MemoryLink {
            a: a.to_string(),
            b: b.to_string(),
            weight,
            created_at: now,
        }
    }

    /// The stable storage key for this pair, `"<a>\u{1f}<b>"`.
    pub fn key(&self) -> String {
        format!("{}\u{1f}{}", self.a, self.b)
    }

    /// The endpoint that is not `id`, if `id` is part of this link.
    pub fn other(&self, id: &str) -> Option<&str> {
        if self.a == id {
            Some(&self.b)
        } else if self.b == id {
            Some(&self.a)
        } else {
            None
        }
    }
}

/// FNV-1a over bytes. Implemented here (rather than using `DefaultHasher`) so
/// the embedding is byte-for-byte reproducible across Rust versions and
/// machines — the whole point of a deterministic, portable memory store.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Add a hashed token into the feature vector using the signed hashing trick:
/// the low bits pick a bucket, the top bit picks a sign. Signing cancels some
/// collision bias so unrelated notes don't drift toward a spuriously high score.
fn add_token(vec: &mut [f32; EMBED_DIM], token: &[u8]) {
    let h = fnv1a(token);
    let idx = (h % EMBED_DIM as u64) as usize;
    let sign = if (h >> 63) & 1 == 1 { 1.0 } else { -1.0 };
    vec[idx] += sign;
}

/// Deterministic lexical embedding of `text`.
///
/// Features are lowercased word unigrams plus character trigrams over the
/// whitespace-collapsed string. Word features capture exact term overlap;
/// trigrams add robustness to small typos and shared word stems. The result is
/// L2-normalized so [`cosine`] reduces to a dot product and every vector is
/// directly comparable. Empty/whitespace input yields the zero vector.
pub fn embed(text: &str) -> Vec<f32> {
    let mut vec = [0.0f32; EMBED_DIM];
    let lower = text.to_lowercase();

    // Word unigrams: split on anything that isn't alphanumeric.
    for word in lower.split(|c: char| !c.is_alphanumeric()) {
        if !word.is_empty() {
            add_token(&mut vec, format!("w:{word}").as_bytes());
        }
    }

    // Character trigrams over the collapsed-whitespace form, so multi-word
    // phrases still contribute cross-word context.
    let collapsed: String = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = collapsed.chars().collect();
    if chars.len() >= 3 {
        for window in chars.windows(3) {
            let tri: String = window.iter().collect();
            add_token(&mut vec, format!("t:{tri}").as_bytes());
        }
    } else if !chars.is_empty() {
        // Very short strings have no trigram; index the whole thing so they are
        // still retrievable rather than becoming the zero vector.
        add_token(&mut vec, format!("t:{collapsed}").as_bytes());
    }

    l2_normalize(&mut vec);
    vec.to_vec()
}

/// Normalize a vector to unit L2 length in place. A zero vector is left as-is.
fn l2_normalize(vec: &mut [f32; EMBED_DIM]) {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in vec.iter_mut() {
            *x /= norm;
        }
    }
}

/// Cosine similarity of two embeddings, in `[-1, 1]` (in practice `[0, 1]` for
/// these non-negative-overlap features). Returns 0 if either vector is degenerate
/// or the lengths differ. Safe on the zero vector.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Options for [`search`].
#[derive(Clone, Debug)]
pub struct SearchOptions {
    /// If set, only memories carrying this tag are considered — the "focus"
    /// filter for distraction-free, project-scoped recall.
    pub focus_tag: Option<String>,
    /// Include archived (soft-forgotten) memories. Off by default.
    pub include_archived: bool,
    /// Maximum hits to return.
    pub limit: usize,
    /// Drop hits scoring below this similarity. Keeps unrelated notes out.
    pub min_score: f32,
}

impl Default for SearchOptions {
    fn default() -> Self {
        SearchOptions {
            focus_tag: None,
            include_archived: false,
            limit: 10,
            min_score: 0.05,
        }
    }
}

/// A scored search result, referring to a memory by its index in the input slice
/// so the caller keeps ownership and nothing is cloned in the hot path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchHit {
    /// Index into the `memories` slice passed to [`search`].
    pub index: usize,
    /// Cosine similarity of the memory to the query.
    pub score: f32,
}

/// Rank `memories` against `query` by lexical similarity.
///
/// Pure and read-only: it never mutates access bookkeeping. Results are sorted
/// by score descending, breaking ties by most-recently-accessed then id so the
/// order is fully deterministic. Archived memories and memories outside the
/// focus tag are excluded per `opts`.
pub fn search(memories: &[Memory], query: &str, opts: &SearchOptions) -> Vec<SearchHit> {
    let q = embed(query);
    let mut hits: Vec<(SearchHit, u64, &str)> = Vec::new();
    for (index, mem) in memories.iter().enumerate() {
        if mem.archived && !opts.include_archived {
            continue;
        }
        if let Some(tag) = &opts.focus_tag
            && !mem.has_tag(tag)
        {
            continue;
        }
        let score = cosine(&q, &embed(&mem.content));
        if score >= opts.min_score {
            hits.push((SearchHit { index, score }, mem.last_accessed, &mem.id));
        }
    }
    hits.sort_by(|x, y| {
        y.0.score
            .partial_cmp(&x.0.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(y.1.cmp(&x.1))
            .then(x.2.cmp(y.2))
    });
    hits.truncate(opts.limit);
    hits.into_iter().map(|(h, _, _)| h).collect()
}

/// Options controlling automatic link discovery.
#[derive(Clone, Copy, Debug)]
pub struct LinkOptions {
    /// Minimum similarity for two memories to be linked.
    pub threshold: f32,
    /// Maximum links to form for a newly added memory.
    pub max_links: usize,
}

impl Default for LinkOptions {
    fn default() -> Self {
        LinkOptions {
            threshold: 0.25,
            max_links: 5,
        }
    }
}

/// Discover the strongest links for `new_content` against `existing` memories.
///
/// Returns `(index_into_existing, weight)` for the top matches above the
/// threshold, so the caller (storage layer) can turn indices into ids. Archived
/// memories are never linked. Pure: forms no edges itself.
pub fn auto_links(new_content: &str, existing: &[Memory], opts: &LinkOptions) -> Vec<(usize, f32)> {
    let q = embed(new_content);
    let mut scored: Vec<(usize, f32)> = existing
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.archived)
        .map(|(i, m)| (i, cosine(&q, &embed(&m.content))))
        .filter(|(_, s)| *s >= opts.threshold)
        .collect();
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    scored.truncate(opts.max_links);
    scored
}

/// Options controlling automatic archival ("old unused memories archived").
#[derive(Clone, Copy, Debug)]
pub struct DecayOptions {
    /// A memory is a candidate only if it has not been accessed within this many
    /// seconds.
    pub max_idle_secs: u64,
    /// ...and only if it has been accessed fewer than this many times. Frequently
    /// used memories are kept regardless of age.
    pub min_access_count: u32,
}

impl Default for DecayOptions {
    fn default() -> Self {
        DecayOptions {
            // 90 days idle...
            max_idle_secs: 90 * 24 * 60 * 60,
            // ...and rarely used.
            min_access_count: 2,
        }
    }
}

/// Indices of memories that are stale enough to archive under `opts`.
///
/// Pure: it selects candidates but archives nothing. Already-archived memories
/// are skipped. `now` is epoch seconds. If `now` predates a memory's
/// `last_accessed` (clock skew), the idle window is treated as zero, so nothing
/// is archived by accident.
pub fn archival_candidates(memories: &[Memory], now: u64, opts: &DecayOptions) -> Vec<usize> {
    memories
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.archived)
        .filter(|(_, m)| m.access_count < opts.min_access_count)
        .filter(|(_, m)| now.saturating_sub(m.last_accessed) > opts.max_idle_secs)
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: &str, content: &str, tags: &[&str]) -> Memory {
        Memory::new(
            id,
            content,
            tags.iter().map(|s| s.to_string()).collect(),
            MemorySource::Manual,
            1_000,
        )
    }

    #[test]
    fn embedding_is_deterministic_and_normalized() {
        let a = embed("the quarterly finance report");
        let b = embed("the quarterly finance report");
        assert_eq!(a, b, "same text must embed identically");
        assert_eq!(a.len(), EMBED_DIM);
        let norm: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4, "expected unit norm, got {norm}");
    }

    #[test]
    fn empty_text_is_zero_vector_and_safe() {
        let z = embed("   ");
        assert!(z.iter().all(|x| *x == 0.0));
        // cosine against a zero vector must not divide by zero.
        assert_eq!(cosine(&z, &embed("anything")), 0.0);
    }

    #[test]
    fn cosine_length_mismatch_is_zero() {
        assert_eq!(cosine(&[1.0, 0.0], &[1.0]), 0.0);
    }

    #[test]
    fn similar_text_scores_higher_than_unrelated() {
        let q = embed("send the invoice to the client");
        let close = cosine(&q, &embed("email the invoice to our client"));
        let far = cosine(&q, &embed("restart the bluetooth speaker firmware"));
        assert!(
            close > far,
            "related text ({close}) should beat unrelated ({far})"
        );
        assert!(close > 0.2, "related score too low: {close}");
    }

    #[test]
    fn search_ranks_by_meaning_and_respects_limit() {
        let memories = vec![
            mem("m1", "quarterly finance report for Q3", &["finance"]),
            mem("m2", "buy milk and eggs from the store", &["home"]),
            mem("m3", "finance report numbers need review", &["finance"]),
        ];
        let hits = search(
            &memories,
            "finance report",
            &SearchOptions {
                limit: 2,
                ..Default::default()
            },
        );
        assert_eq!(hits.len(), 2, "limit must cap results");
        // The two finance notes should outrank the groceries note.
        let ids: Vec<&str> = hits.iter().map(|h| memories[h.index].id.as_str()).collect();
        assert!(ids.contains(&"m1"));
        assert!(ids.contains(&"m3"));
        assert!(!ids.contains(&"m2"));
    }

    #[test]
    fn focus_tag_filters_to_a_project() {
        let memories = vec![
            mem("m1", "ship the release notes", &["ghost"]),
            mem("m2", "ship the release notes", &["sidequest"]),
        ];
        let hits = search(
            &memories,
            "release notes",
            &SearchOptions {
                focus_tag: Some("Ghost".to_string()), // case-insensitive
                ..Default::default()
            },
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(memories[hits[0].index].id, "m1");
    }

    #[test]
    fn archived_excluded_unless_requested() {
        let mut memories = vec![mem("m1", "renew the domain certificate", &[])];
        memories[0].archived = true;

        let hidden = search(&memories, "renew certificate", &SearchOptions::default());
        assert!(hidden.is_empty(), "archived memory must be hidden");

        let shown = search(
            &memories,
            "renew certificate",
            &SearchOptions {
                include_archived: true,
                ..Default::default()
            },
        );
        assert_eq!(shown.len(), 1);
    }

    #[test]
    fn auto_links_connects_related_and_skips_archived() {
        let mut existing = vec![
            mem("m1", "deploy the ghost organizer release", &[]),
            mem("m2", "unrelated note about sourdough bread", &[]),
            mem("m3", "ghost organizer release checklist", &[]),
        ];
        existing[2].archived = true; // archived should not be linked

        let links = auto_links(
            "ghost organizer release plan",
            &existing,
            &LinkOptions::default(),
        );
        let linked_indices: Vec<usize> = links.iter().map(|(i, _)| *i).collect();
        assert!(linked_indices.contains(&0), "should link the related note");
        assert!(
            !linked_indices.contains(&1),
            "should not link the unrelated note"
        );
        assert!(
            !linked_indices.contains(&2),
            "must never link an archived note"
        );
    }

    #[test]
    fn auto_links_respects_max_links() {
        let existing: Vec<Memory> = (0..10)
            .map(|i| mem(&format!("m{i}"), "ghost organizer release", &[]))
            .collect();
        let links = auto_links(
            "ghost organizer release",
            &existing,
            &LinkOptions {
                threshold: 0.1,
                max_links: 3,
            },
        );
        assert_eq!(links.len(), 3);
    }

    #[test]
    fn memory_link_is_canonical_and_navigable() {
        let l1 = MemoryLink::new("b", "a", 0.5, 1);
        let l2 = MemoryLink::new("a", "b", 0.5, 1);
        assert_eq!(l1.key(), l2.key(), "pair order must not matter");
        assert_eq!(l1.a, "a");
        assert_eq!(l1.other("a"), Some("b"));
        assert_eq!(l1.other("b"), Some("a"));
        assert_eq!(l1.other("z"), None);
    }

    #[test]
    fn archival_candidates_select_old_and_unused_only() {
        let day = 24 * 60 * 60;
        let now = 1_000 * day;
        let opts = DecayOptions {
            max_idle_secs: 90 * day,
            min_access_count: 2,
        };

        let mut old_unused = mem("old", "stale note", &[]);
        old_unused.last_accessed = now - 100 * day;
        old_unused.access_count = 0;

        let mut old_but_used = mem("used", "important note", &[]);
        old_but_used.last_accessed = now - 100 * day;
        old_but_used.access_count = 5;

        let mut recent = mem("recent", "fresh note", &[]);
        recent.last_accessed = now - day;
        recent.access_count = 0;

        let mut already = mem("archived", "gone note", &[]);
        already.last_accessed = now - 100 * day;
        already.archived = true;

        let memories = vec![old_unused, old_but_used, recent, already];
        let candidates = archival_candidates(&memories, now, &opts);
        assert_eq!(candidates, vec![0], "only the old, unused, live note");
    }

    #[test]
    fn archival_is_safe_under_clock_skew() {
        let memories = vec![{
            let mut m = mem("m", "note", &[]);
            m.last_accessed = 10_000;
            m
        }];
        // now < last_accessed: saturating_sub -> 0 idle, nothing archived.
        let candidates = archival_candidates(&memories, 5_000, &DecayOptions::default());
        assert!(candidates.is_empty());
    }
}
