//! Canonical execution runtime for Ghost 2.0.

pub mod execute;
pub mod fs;
pub mod receipt;
pub mod ui;
pub mod verify;

pub use execute::{execute_action_plan_with_progress, RuntimeResult};
pub use receipt::{build_receipt, ExecutionReceipt, ReceiptStep};
pub use verify::{StepVerification, VerificationStatus};
