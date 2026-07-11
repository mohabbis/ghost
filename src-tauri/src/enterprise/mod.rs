//! Enterprise operations primitives for approved, auditable workflows.
//!
//! This module is intentionally data-model first. It does not expose Tauri
//! commands or background monitoring. Enterprise runs still follow Ghost's
//! trust pipeline: intent, plan, policy check, approval, deterministic
//! execution, audit, and undo.

pub mod approvals;
pub mod exceptions;
pub mod playbooks;
pub mod reports;
pub mod scheduling;
pub mod tenancy;
pub mod workflows;
