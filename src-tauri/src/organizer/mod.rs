//! Ghost Organizer: plan, then safely execute, file-organization work.
//!
//! The [`planner`] generates a **reviewable plan and mutates nothing** — it
//! scans an approved Zone's source folders, classifies each file
//! deterministically, proposes a safe target, detects conflicts, expresses
//! every proposed change as a [`crate::policy::Capability`], and runs each one
//! through [`crate::policy::evaluate`], yielding an [`OrganizerPlan`] preview.
//!
//! Once a user approves that plan, the [`executor`] applies it: re-checking
//! policy per action, verifying state, writing undo data before each mutation,
//! and recording an audit event for every action. The [`undo`] runner replays
//! that journal in reverse to roll the changes back.
//!
//! ```text
//! Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
//!          ^^^^^^^^^^^^^^^^^              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//!          planner (read-only)           executor + undo (this module)
//! ```
//!
//! Reuses the trust primitives from `crate::policy`, the audit/undo records from
//! `crate::audit`, and loads boundaries from `crate::storage`. The planner is
//! read-only; only the executor and undo runner touch the filesystem.

pub mod classifier;
pub mod conflict;
pub mod executor;
pub mod file_identity;
pub mod naming;
pub mod pipeline;
pub mod planner;
pub mod scanner;
pub mod undo;

#[cfg(test)]
pub(crate) mod testutil;

pub use classifier::{Category, Classification};
pub use conflict::Conflict;
pub use executor::{ExecutionReport, execute_plan};
pub use pipeline::{ZoneRunOutcome, execute_zone, undo_zone_run};
pub use planner::{OrganizerPlan, PlanAction, PlanSummary, SkipReason, SkippedFile, plan_zone};
pub use scanner::ScannedFile;
pub use undo::{UndoReport, revert};
