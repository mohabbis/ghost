//! Canonical execution runtime for Ghost 2.0.

pub mod execute;
pub mod fs;
pub mod persist;
pub mod receipt;
pub mod semantic;
pub mod ui;
pub mod verify;

pub use execute::{
    execute_action_plan_with_progress, execute_action_plan_with_reliability, RuntimeResult,
};
pub use persist::{run_persisted_action_plan, PersistedRunOutcome};
pub use receipt::{build_receipt, ExecutionReceipt, ReceiptStep};
pub use semantic::{ResolvedTarget, SemanticError, UiTarget};
pub use verify::{StepVerification, VerificationStatus};
