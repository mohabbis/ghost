//! Deterministic token compression for LLM-bound content.
//!
//! Before any text reaches a model — an accessibility tree, a scraped page, an
//! email body — it is run through this layer to strip noise that costs tokens
//! but carries little signal. The transform is **pure and deterministic**
//! (same input → same output, no model, no network), which keeps it inside
//! Ghost's trust model: it is auditable, testable, and reduces how much of the
//! user's content ever leaves the machine.
//!
//! The pipeline, in order:
//!
//! 1. **HTML → text** — drop tags, decode common entities, turn block elements
//!    into line breaks (only when the input actually looks like HTML).
//! 2. **Shorten URLs** — collapse long links to `scheme://host/first-seg/…`.
//! 3. **De-duplicate lines** — remove consecutive repeats always, and
//!    non-consecutive exact repeats for non-trivial lines.
//! 4. **Collapse whitespace** — trim trailing spaces, squeeze blank runs.
//! 5. **Truncate to a token budget** (optional) — on a `char` boundary so a
//!    multi-byte sequence (CJK, emoji) is never split mid-codepoint.
//!
//! Token counts are a deterministic estimate (~4 chars/token), good enough to
//! bound a prompt and report a reduction; they are not a tokenizer.

use regex::Regex;
use std::sync::OnceLock;

/// Rough chars-per-token used for budgeting and reporting. Real tokenizers vary
/// by model and language; this is a stable, dependency-free approximation.
const CHARS_PER_TOKEN: usize = 4;

/// Lines shorter than this are exempt from *non-consecutive* de-duplication, so
/// structural punctuation (`{`, `]`, `--`) and short repeated values survive.
const DEDUPE_MIN_LEN: usize = 12;

/// URLs longer than this get shortened; shorter ones are left untouched.
const URL_SHORTEN_THRESHOLD: usize = 48;

/// What a single [`compress_to_budget`] call did, for logging / reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressionStats {
    pub original_chars: usize,
    pub compressed_chars: usize,
    pub original_tokens_est: usize,
    pub compressed_tokens_est: usize,
}

impl CompressionStats {
    /// Fraction of estimated tokens removed, in `0.0..=1.0`. Zero when the input
    /// was empty or grew (which the pipeline avoids, but be safe).
    pub fn reduction_ratio(&self) -> f32 {
        if self.original_tokens_est == 0 || self.compressed_tokens_est >= self.original_tokens_est {
            return 0.0;
        }
        let saved = self.original_tokens_est - self.compressed_tokens_est;
        saved as f32 / self.original_tokens_est as f32
    }
}

/// Estimate the token count of `s` (deterministic, ~[`CHARS_PER_TOKEN`] chars
/// per token, counting Unicode scalar values rather than bytes).
pub fn estimate_tokens(s: &str) -> usize {
    s.chars().count().div_ceil(CHARS_PER_TOKEN)
}

/// Compress `input` with no length budget — clean it up without truncating.
pub fn compress(input: &str) -> String {
    let (out, _) = compress_to_budget(input, usize::MAX);
    out
}

/// Compress `input` and truncate the result to at most `max_tokens` estimated
/// tokens, returning the text and the [`CompressionStats`] for the call.
///
/// Truncation happens on a `char` boundary, so a multi-byte codepoint (CJK
/// ideograph, emoji) is never cut in half. When truncated, a short `… [truncated]`
/// marker is appended so a downstream model can tell the content was clipped.
pub fn compress_to_budget(input: &str, max_tokens: usize) -> (String, CompressionStats) {
    let original_chars = input.chars().count();
    let original_tokens_est = estimate_tokens(input);

    let mut text = if looks_like_html(input) {
        html_to_text(input)
    } else {
        input.to_string()
    };
    text = shorten_urls(&text);
    text = dedupe_lines(&text);
    text = collapse_whitespace(&text);
    text = truncate_to_tokens(&text, max_tokens);

    let stats = CompressionStats {
        original_chars,
        compressed_chars: text.chars().count(),
        original_tokens_est,
        compressed_tokens_est: estimate_tokens(&text),
    };
    (text, stats)
}

/// Heuristic: does this string contain HTML worth stripping? Looks for a `<`
/// immediately followed by a letter or `/` (an opening or closing tag).
fn looks_like_html(s: &str) -> bool {
    let bytes = s.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b'<' {
            let next = bytes[i + 1];
            if next == b'/' || next.is_ascii_alphabetic() {
                return true;
            }
        }
    }
    false
}

/// Tags that should become a line break (their content sits on its own line).
const BLOCK_TAGS: &[&str] = &[
    "p",
    "br",
    "div",
    "li",
    "ul",
    "ol",
    "tr",
    "table",
    "section",
    "article",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "footer",
    "blockquote",
    "pre",
];

/// Strip HTML to plain text: drop tags (their whole content for `script`/`style`),
/// decode a handful of common entities, and break lines on block-level tags.
fn html_to_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();

    while let Some((_, c)) = chars.next() {
        if c != '<' {
            out.push(c);
            continue;
        }
        // Collect the tag body up to the closing '>'.
        let mut tag = String::new();
        for (_, tc) in chars.by_ref() {
            if tc == '>' {
                break;
            }
            tag.push(tc);
        }
        let tag_trim = tag.trim();
        let name = tag_name(tag_trim);

        // Drop the entire contents of script/style blocks.
        if name == "script" || name == "style" {
            skip_until_close(&mut chars, &name);
            continue;
        }
        if BLOCK_TAGS.contains(&name.as_str()) {
            out.push('\n');
        }
        // Any other tag is simply removed (its text content stays).
    }

    decode_entities(&out)
}

/// The lowercase element name from a tag body, e.g. `/DIV class=x` -> `div`.
fn tag_name(tag_body: &str) -> String {
    tag_body
        .trim_start_matches('/')
        .split([' ', '\t', '\n', '/'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Advance `chars` until the matching `</name>` is consumed (for script/style).
fn skip_until_close(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, name: &str) {
    let needle = format!("/{name}");
    while let Some((_, c)) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            for (_, tc) in chars.by_ref() {
                if tc == '>' {
                    break;
                }
                tag.push(tc);
            }
            if tag.trim().to_ascii_lowercase().starts_with(&needle) {
                return;
            }
        }
    }
}

/// Decode the small set of HTML entities that actually show up in scraped text.
fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// Shorten URLs longer than [`URL_SHORTEN_THRESHOLD`] to `scheme://host/seg/…`,
/// dropping the query, fragment, and deep path that rarely help a model.
fn shorten_urls(text: &str) -> String {
    static URL_RE: OnceLock<Regex> = OnceLock::new();
    let re = URL_RE.get_or_init(|| Regex::new(r"https?://[^\s)<>\]]+").unwrap());

    re.replace_all(text, |caps: &regex::Captures| {
        let url = &caps[0];
        if url.chars().count() <= URL_SHORTEN_THRESHOLD {
            return url.to_string();
        }
        shorten_one_url(url)
    })
    .into_owned()
}

fn shorten_one_url(url: &str) -> String {
    // Split scheme://rest, then host / first path segment.
    let (scheme, rest) = match url.split_once("://") {
        Some((s, r)) => (s, r),
        None => return url.to_string(),
    };
    let mut parts = rest.splitn(2, '/');
    let host = parts.next().unwrap_or("");
    match parts.next() {
        Some(path) if !path.is_empty() => {
            let first_seg = path.split(['/', '?', '#']).next().unwrap_or("");
            format!("{scheme}://{host}/{first_seg}/…")
        }
        _ => format!("{scheme}://{host}/…"),
    }
}

/// Remove duplicate lines: always collapse consecutive repeats; for lines at
/// least [`DEDUPE_MIN_LEN`] long, also drop later exact duplicates anywhere.
fn dedupe_lines(text: &str) -> String {
    let mut seen_long: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut prev: Option<&str> = None;
    let mut out: Vec<&str> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim_end();
        if prev == Some(trimmed) {
            continue; // consecutive duplicate
        }
        if trimmed.chars().count() >= DEDUPE_MIN_LEN && !seen_long.insert(trimmed) {
            continue; // non-consecutive duplicate of a substantial line
        }
        out.push(trimmed);
        prev = Some(trimmed);
    }
    out.join("\n")
}

/// Squeeze runs of blank lines to a single blank line and trim the ends.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0usize;

    for line in text.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

/// Truncate to at most `max_tokens` estimated tokens, on a `char` boundary.
fn truncate_to_tokens(text: &str, max_tokens: usize) -> String {
    if max_tokens == usize::MAX {
        return text.to_string();
    }
    let max_chars = max_tokens.saturating_mul(CHARS_PER_TOKEN);
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    const MARKER: &str = "… [truncated]";
    // Reserve room for the marker so the result still respects the budget.
    let keep = max_chars.saturating_sub(MARKER.chars().count()).max(1);
    let mut truncated: String = text.chars().take(keep).collect();
    truncated.push_str(MARKER);
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(compress(""), "");
        let (out, stats) = compress_to_budget("", 100);
        assert_eq!(out, "");
        assert_eq!(stats.reduction_ratio(), 0.0);
    }

    #[test]
    fn strips_html_tags_and_decodes_entities() {
        let html = "<div><p>Hello &amp; welcome</p><p>Line&nbsp;two &lt;ok&gt;</p></div>";
        let out = compress(html);
        assert!(out.contains("Hello & welcome"));
        assert!(out.contains("Line two <ok>"));
        assert!(!out.contains('<') || out.contains("<ok>")); // only the decoded entity
        assert!(!out.contains("&amp;"));
    }

    #[test]
    fn drops_script_and_style_contents() {
        let html =
            "<p>keep</p><script>var secret = 1;</script><style>.x{color:red}</style><p>also</p>";
        let out = compress(html);
        assert!(out.contains("keep"));
        assert!(out.contains("also"));
        assert!(!out.contains("secret"));
        assert!(!out.contains("color:red"));
    }

    #[test]
    fn plain_text_without_tags_is_not_mangled() {
        let text = "a < b and c > d are comparisons, not tags";
        // No tag-shaped `<x`/`</`, so the HTML path is skipped entirely.
        assert_eq!(compress(text), text);
    }

    #[test]
    fn shortens_long_urls_but_keeps_short_ones() {
        let short = "see https://ex.com/a";
        assert_eq!(compress(short), short);

        let long = "ref https://example.com/very/deep/path/here?token=abcdef123456&page=42#section";
        let out = compress(long);
        assert!(out.contains("https://example.com/very/…"), "got: {out}");
        assert!(!out.contains("token=abcdef"));
    }

    #[test]
    fn dedupes_consecutive_and_repeated_substantial_lines() {
        let text =
            "the quick brown fox\nthe quick brown fox\nthe quick brown fox\nunique tail line";
        let out = compress(text);
        assert_eq!(out.matches("the quick brown fox").count(), 1);
        assert!(out.contains("unique tail line"));
    }

    #[test]
    fn keeps_short_repeated_structural_lines() {
        // Short lines (< DEDUPE_MIN_LEN) are only collapsed when *consecutive*,
        // so non-adjacent structural punctuation survives.
        let text = "{\n  \"a\": 1\n}\n{\n  \"b\": 2\n}";
        let out = compress(text);
        assert_eq!(out.matches('{').count(), 2);
        assert_eq!(out.matches('}').count(), 2);
    }

    #[test]
    fn collapses_blank_line_runs() {
        let text = "first\n\n\n\nsecond";
        assert_eq!(compress(text), "first\n\nsecond");
    }

    #[test]
    fn preserves_multibyte_text_and_never_splits_a_codepoint() {
        // CJK + emoji must survive compression and budget truncation intact.
        let text = "日本語のテキスト 😀🎉 and more 日本語";
        let out = compress(text);
        assert!(out.contains("日本語"));
        assert!(out.contains("😀🎉"));

        let (clipped, _) = compress_to_budget(&"あ".repeat(100), 10);
        // Valid UTF-8 (no panic constructing it) and within budget + marker.
        assert!(clipped.contains('あ'));
        assert!(clipped.contains("[truncated]"));
    }

    #[test]
    fn truncation_respects_the_token_budget() {
        let big = "word ".repeat(1000); // ~5000 chars
        let (out, stats) = compress_to_budget(&big, 50);
        // 50 tokens * 4 chars/token = 200 char ceiling.
        assert!(out.chars().count() <= 200, "len {}", out.chars().count());
        assert!(stats.compressed_tokens_est <= 50);
        assert!(stats.reduction_ratio() > 0.5);
    }

    #[test]
    fn no_budget_does_not_truncate() {
        let big = "content ".repeat(500);
        let out = compress(&big);
        assert!(!out.contains("[truncated]"));
    }

    #[test]
    fn idempotent_on_already_clean_text() {
        let text = "Clean line one\nClean line two\n\nA short paragraph of words.";
        let once = compress(text);
        let twice = compress(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn reports_a_real_reduction_on_noisy_html() {
        let noisy = format!(
            "<html><body>{}<script>junk junk junk</script></body></html>",
            "<p>Repeated paragraph of content.</p>".repeat(20)
        );
        let (_out, stats) = compress_to_budget(&noisy, usize::MAX);
        assert!(
            stats.compressed_tokens_est < stats.original_tokens_est,
            "expected fewer tokens: {stats:?}"
        );
        assert!(stats.reduction_ratio() > 0.3);
    }
}
