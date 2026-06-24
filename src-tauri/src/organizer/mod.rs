//! Ghost Organizer: the file-organization planner.
//!
//! The planner generates a **reviewable plan and mutates nothing**. It scans an
//! approved Zone's source folders, filters system/ignored files, classifies
//! each file deterministically, proposes a safe target folder and filename,
//! detects conflicts, expresses every proposed change as a
//! [`crate::policy::Capability`], and runs each one through
//! [`crate::policy::evaluate`]. The result is an [`OrganizerPlan`] — a preview
//! carrying a [`crate::policy::PolicyDecision`], confidence, and reason per
//! action — for the approval UI and executor that land in later phases.
//!
//! ```text
//! Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
//!          ^^^^^^^^^^^^^^^^^
//!          this module covers Plan + Policy only; it never executes.
//! ```
//!
//! Reuses the trust primitives from `crate::policy` and loads boundaries from
//! `crate::storage`. It performs only read-only filesystem access.

pub mod classifier;
pub mod conflict;
pub mod naming;
pub mod planner;
pub mod scanner;

#[cfg(test)]
mod testutil;

pub use classifier::{Category, Classification};
pub use conflict::Conflict;
pub use planner::{plan_zone, OrganizerPlan, PlanAction, PlanSummary, SkipReason, SkippedFile};
pub use scanner::ScannedFile;
