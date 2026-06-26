# Deterministic event compression

`src-tauri/src/core/compression/` converts a raw `InputEvent` stream into a reviewable [`CompressionReport`] — the first stage of Ghost's trust pipeline:

```text
Raw Input Capture -> Deterministic Compression -> Semantic Timeline -> Guard -> Policy -> Approval -> Execute -> Vault -> Undo
```

It is **pure and deterministic: no LLM, no network**. The same input always yields the same steps, so the output is auditable and testable. This is the read-model the user reviews; it never executes anything, and replay still runs off the raw events.

> Not to be confused with `core/compress.rs`, which compresses *text* before sending it to a model. This module compresses *events* into workflow steps. (`compress` = text; `compression` = events.)

## Module layout

| File | Responsibility |
| --- | --- |
| `types.rs` | `CompressedStep`, `CompressionReport`, `Target`, warnings |
| `grouping.rs` | Low-level classifiers: mouse button codes, modifier bits, scroll/shortcut buckets |
| `confidence.rs` | Target confidence scoring from accessibility fields |
| `redaction.rs` | Redaction policy (reuses `guard::is_sensitive_element`) |
| `compressor.rs` | The single-pass state machine |
| `tests.rs` | Acceptance + invariant tests |

## What it groups

- **Click** — a mouse press + release (`button` codes `0/1`=left, `2/3`=right on both capture backends) fold into one `ClickStep`.
- **TypeText** — a run of typed characters becomes one step. **Redacted by default**: the character count is kept, the content is dropped, and secure fields are never retained even when retention is opted into.
- **Shortcut** — a Command/Control chord (`Cmd+S` -> "Save").
- **Scroll** — a burst merges into one step with a coarse direction + magnitude.
- **Wait** — consecutive delays sum; sub-250 ms noise is dropped, meaningful pauses become a `WaitStep`.
- **Unknown** — anything unclassified is surfaced, never silently dropped.

## Confidence

Targets are scored by how reliably they can be re-found on replay:

| Score | Signal |
| --- | --- |
| 0.95 | stable AX identifier + app + role + name |
| 0.80 | name + role + app |
| 0.55 | app only, plus fallback coordinates |
| 0.25 | coordinates only |

Steps at or below `LOW_CONFIDENCE` (0.5) and coordinate-only / secure-field steps are flagged in `report.warnings` for review.

## Privacy and focus

`InputEvent::Key` carries no element, so a typing run is attributed to the **current focus** — the element of the most recent click — which is also how Ghost Guard decides to suppress keystrokes after a click into a sensitive field. Secure-field detection reuses `guard::is_sensitive_element`, so this layer and Guard agree on what is sensitive.

## API

- `compress(&[InputEvent]) -> CompressionReport` — default, text redacted.
- `compress_with_options(&[InputEvent], keep_text) -> CompressionReport` — retain literal text for non-secure fields. Secure fields are always redacted.

## Status

Sprint 1 is backend module + tests only, following the repo's backend-first pattern: no Tauri command yet. The `compress_workflow` command and compressed-step timeline UI are Sprint 2; routing Ghost Guard through compressed steps is Sprint 3.
