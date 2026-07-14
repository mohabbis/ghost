# Ghost Atlas (semantic memory)

Ghost Atlas is Ghost's local semantic-memory graph: a persistent store of short
notes ("memories") that can be recalled by meaning, linked to related notes
automatically, filtered to a single project for focused recall, and quietly
archived when they go stale.

It is Ghost's adaptation of the "Neural Atlas" idea seen in other assistants (a
memory graph that grows over time and retrieves by meaning). The *concept* was
ported; the *implementation* was rebuilt to fit Ghost's trust model rather than
copied. In particular, Ghost Atlas does **not** download or run a
sentence-transformer model, and it does **not** talk to the network.

## What it is — and is not

| Neural Atlas (source concept) | Ghost Atlas (this port) |
| --- | --- |
| SQLite + sentence-transformer embeddings | redb + a deterministic **lexical** embedding, no model |
| Neural semantic similarity | Lexical (word + character-trigram) similarity |
| "old unused memories archived automatically" | Same, but archival is a reversible flag — never a delete |
| Cloud-capable assistant memory | Local-only, offline, no network, no telemetry of content |

**Honest framing (CLAUDE.md rule 10):** retrieval is lexical, not neural. It
finds notes that share words, word-stems, and character n-grams with the query —
it does not understand synonyms the way a trained embedding model would. Do not
market it as neural/AI semantic memory. A real embedding provider could be added
later behind the `experimental` feature flag (the same pattern as Ghost's other
AI providers), feeding vectors into the same graph; the deterministic lexical
embedding is the trustworthy default.

## Trust-model fit

- **Local and offline.** No network, no cloud, no model download, no ML runtime.
- **Deterministic.** Embeddings are a pure function of the text, so the same
  note always yields the same vector and every result is reproducible and
  unit-testable (no GPU, no fixtures).
- **No silent delete.** "Forgetting" flips an `archived` flag; the row and its
  links are never removed, and `unarchive_memory` restores it. This matches
  Ghost's deny-silent-delete stance.
- **Provenance preserved.** Every memory records a `MemorySource`
  (`manual` / `routine` / `organizer` / `observation` / `import`) so a future
  review surface can tell "the user said" from "the app inferred."
- **No command surface yet.** This first increment is a tested engine plus
  persistence, following the repo's commandless-scaffolding convention
  (`checks/`, `compliance/`, …). Any Tauri command on top must still go through
  the full trust pipeline — see "Wiring a command later."

## Architecture

```text
core/atlas.rs      pure engine: embedding, cosine, search, auto-link, archival
                   selection. No I/O, no network, no persisted vectors.
storage/atlas.rs   redb persistence + the store API that drives the engine over
                   the live corpus. Local-only, never deletes.
```

Two redb tables (created in `storage::mod::init_tables`, so old databases pick
them up automatically on next open):

- `atlas_memories`: `id -> JSON(Memory)`
- `atlas_links`: `"<a>\u{1f}<b>" -> JSON(MemoryLink)` — one row per undirected
  pair, keyed in canonical (sorted) order.

### The embedding

`core::atlas::embed(text) -> Vec<f32>` of length `EMBED_DIM` (256):

1. Lowercase the text.
2. Hash each **word unigram** (`w:<word>`) and each **character trigram**
   (`t:<abc>`) into the vector using the signed hashing trick (FNV-1a picks the
   bucket; its top bit picks the sign, which cancels some collision bias).
3. L2-normalize, so `cosine` is a dot product and all vectors are comparable.

FNV-1a is implemented in-tree (not `DefaultHasher`) specifically so the vector is
byte-for-byte reproducible across Rust versions and machines. **Embeddings are
never persisted** — they are recomputed from `content` on demand, so there is no
stored vector that could desync from the text or need migrating.

### Retrieval, linking, decay

- **Search** (`atlas::search`) embeds the query, scores every candidate by
  cosine similarity, drops anything below `min_score`, and sorts by score
  (ties broken by recency then id, so ordering is fully deterministic).
- **Focus filter** (`SearchOptions::focus_tag`) restricts search to memories
  carrying one tag — distraction-free, project-scoped recall.
- **Auto-linking** (`atlas::auto_links`) runs on insert: the new note is linked
  to its strongest matches above `LinkOptions::threshold` (default 0.25, up to 5
  links). Archived memories are never linked.
- **Archival** (`atlas::archival_candidates`) selects live memories that are
  both old (idle longer than `DecayOptions::max_idle_secs`, default 90 days) and
  rarely used (`access_count < min_access_count`, default 2). Frequently used
  notes are kept regardless of age. `storage::atlas::run_archival` applies one
  pass and returns the archived ids so the action can be audited.

### Store API (`storage::atlas`)

| Function | Touches | Notes |
| --- | --- | --- |
| `add_memory` | local db (write) | embeds, auto-links, one write txn |
| `get_memory` / `list_memories` | local db (read) | |
| `search` | local db (read) | read-only; does **not** bump access counts |
| `neighbors` | local db (read) | linked memories, strongest first |
| `record_access` | local db (write) | call with ids you actually surface |
| `archive_memory` / `unarchive_memory` | local db (write) | reversible soft-forget |
| `run_archival` | local db (write) | maintenance pass; returns archived ids |

None of these touch files outside the redb database, OS input, screenshots, the
network, or authentication/secrets.

`search` is deliberately read-only: "was used" should reflect real user-facing
recall, so the caller invokes `record_access` for the ids it actually shows,
rather than every background scan inflating the counts that drive archival.

## Scale

This is a personal store (hundreds to low thousands of notes). Search and
auto-link load the live corpus and embed each note's content on the fly — cheap
and deterministic at that scale, and it keeps storage lean (no stored vectors).
If the corpus ever grows large enough to matter, add an index in
`storage/atlas.rs`; the engine does not change.

## Wiring a command later

Exposing this to the UI means adding Tauri commands, which per CLAUDE.md must
each carry a module + risk class, a policy check, an approval step where the
action mutates, audit/undo behavior, and a `docs/command-registry.md` entry.
Suggested surface and risk framing:

- `atlas_add_memory`, `atlas_search`, `atlas_neighbors`, `atlas_list` — local db
  only; low risk. `add` is a reversible local write (archive to undo).
- `atlas_archive` / `atlas_unarchive` — reversible; low risk.
- `atlas_run_archival` — maintenance; return the archived ids for the audit log.

Search and read commands are read-only and local; the add/archive commands are
reversible local writes. There is no network, screen capture, or secret access
anywhere in this module, and there must not be.

## Tests

`core/atlas.rs` and `storage/atlas.rs` each carry unit tests
(`cargo test --manifest-path src-tauri/Cargo.toml atlas`) covering: deterministic
and normalized embeddings, zero-vector/length-mismatch safety, related-beats-
unrelated ranking, the focus filter, archived exclusion, auto-linking (including
skipping archived and honoring `max_links`), reversible archive/unarchive,
access bookkeeping, and a full add → link → search → archive round-trip against
an in-memory redb.
