//! Scoped enterprise financial-operations boundary.
//!
//! This module is reserved for deterministic, local-first helpers. It must not
//! approve financial decisions, transmit funds, or bypass Ghost's policy and
//! approval pipeline.
//!
//! [`reconcile`] is the pure month-end close reconciliation core: it compares a
//! per-client expected-documents checklist against the documents already
//! classified as present and reports what is met, missing, or unexpected. It is
//! the reusable heart of the accounting-close vertical (see
//! `docs/vertical-accounting-close.md`) and, like everything here, stays
//! commandless — a Tauri surface on top must still carry its own module, risk
//! class, policy check, approval, and audit/undo behavior.

pub mod reconcile;

pub use reconcile::{
    CloseChecklist, ExpectedDoc, PresentDoc, ReconcileReport, ReconcileStatus, RequirementResult,
    reconcile,
};
