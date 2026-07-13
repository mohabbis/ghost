//! Shared Action Plan IR — the only reviewable executable representation (v1).

pub mod compile;
pub mod demo;
pub mod types;

pub use compile::{from_compression_report, from_organizer_plan};
pub use demo::build_invoice_demo;
pub use types::*;
