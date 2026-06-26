//! Deterministic **event** compression: raw input -> reviewable workflow steps.
//!
//! This is the foundation of Ghost's trust pipeline:
//!
//! ```text
//! Raw Input Capture -> [Deterministic Compression] -> Semantic Timeline -> ...
//! ```
//!
//! It converts a noisy [`InputEvent`](crate::core::events::InputEvent) stream
//! into a compact [`CompressionReport`]: a handful of [`CompressedStep`]s with
//! confidence scores, redacted typing, and reviewer warnings. It is pure and
//! deterministic: **no LLM, no network**.
//!
//! Privacy is the default: typed text is redacted unless a caller explicitly
//! opts out, and secure fields are never retained. Secure-field detection reuses
//! [`crate::core::guard::is_sensitive_element`].
//!
//! Not to be confused with [`crate::core::compress`], which compresses *text*
//! before it is sent to a model; this module compresses *events* into workflow
//! steps.

mod compressor;
mod confidence;
mod grouping;
mod redaction;
mod types;

#[cfg(test)]
mod tests;

pub use compressor::{compress, compress_with_options};
pub use types::{
    ClickButton, ClickStep, CompressedStep, CompressionReport, CompressionWarning, ScrollDirection,
    ScrollMagnitude, ScrollStep, ShortcutStep, Target, TypeTextStep, UnknownStep, WaitStep,
    LOW_CONFIDENCE,
};
