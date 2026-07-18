//! Canonical execution runtime for Ghost 2.0.

pub mod execute;
pub mod fs;
pub mod locator;
pub mod persist;
pub mod receipt;
pub mod semantic;
pub mod ui;
pub mod verify;

pub use execute::{
    RuntimeResult, execute_action_plan_with_progress, execute_action_plan_with_reliability,
};
pub use locator::{AxConstraint, AxQuality, Locator, score_ax_candidate};
pub use persist::{PersistedRunOutcome, run_persisted_action_plan};
pub use receipt::{ExecutionReceipt, ReceiptStep, build_receipt};
pub use semantic::{ResolvedTarget, SemanticError, UiTarget};
pub use verify::{StepVerification, VerificationStatus};
