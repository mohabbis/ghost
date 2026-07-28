# Token compression

`src-tauri/src/core/compress.rs` is a deterministic preprocessing layer for any
text that is about to be sent to an LLM — today, the accessibility-tree context
in `core/llm.rs`; later, scraped pages or document bodies for the experimental
AI surfaces.

## Why it fits Ghost's trust model

It is **pure and deterministic** — same input, same output, no model, no network.
That keeps it auditable and testable, and it has a privacy payoff as well as a
cost one: by stripping boilerplate before content leaves the machine, it reduces
*how much of the user's data is ever transmitted* to a provider. (The idea is
borrowed from OpenHuman's "TokenJuice"; the implementation here is Ghost's own,
dependency-free, and wired only into already-opt-in AI paths.)

## Pipeline

In order (`compress_to_budget`):

1. **HTML → text** — drop tags, decode common entities, break lines on block
   elements, and discard `<script>`/`<style>` contents. Runs only when the input
   actually looks like HTML, so plain text and code are left untouched.
2. **Shorten URLs** — links over 48 chars collapse to `scheme://host/first-seg/…`,
   dropping query strings and fragments that rarely help a model.
3. **De-duplicate lines** — consecutive repeats are always removed; non-adjacent
   exact repeats are removed only for substantial lines (≥12 chars), so
   structural punctuation and short repeated values survive.
4. **Collapse whitespace** — trim trailing spaces and squeeze blank-line runs.
5. **Truncate to a token budget** (optional) — on a `char` boundary, so a
   multi-byte codepoint (CJK, emoji) is never split; a `… [truncated]` marker is
   appended when clipping happens.

Token counts are a deterministic estimate (~4 chars/token), used to bound a
prompt and report a reduction — not a real tokenizer.

## API

- `compress(&str) -> String` — clean up, no truncation.
- `compress_to_budget(&str, max_tokens) -> (String, CompressionStats)` — clean up
  and clip to a token ceiling, returning stats (`reduction_ratio()` etc.).
- `estimate_tokens(&str) -> usize`.

## Where it's wired

`core/llm.rs` compresses the accessibility tree to `AX_CONTEXT_TOKEN_BUDGET`
(2000 tokens) before embedding it in the OpenAI and Claude prompts. These are
experimental, opt-in AI paths (`commands/experimental.rs`); the stable core does
not call a model. Unit tests live in the module and cover HTML stripping,
entity decoding, URL shortening, de-duplication, multibyte safety, budget
truncation, and idempotence.

## Honest scope

Truncation is **codepoint**-safe, not full grapheme-cluster safe: a ZWJ emoji
sequence could in principle be split at a cluster boundary (never mid-codepoint,
so the output is always valid UTF-8). Making it cluster-safe would mean adding a
`unicode-segmentation` dependency — deferred until there's a concrete need.
