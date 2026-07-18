//! Canonical execution runtime for Ghost 2.0.

pub mod capture;
pub mod evidence;
pub mod execute;
pub mod fs;
pub mod helper_budget;
pub mod locator;
pub mod persist;
pub mod receipt;
pub mod semantic;
pub mod ui;
pub mod verify;
pub mod vision_fallback;

pub use capture::{
    CaptureError, StillFrame, StreamCaptureOpts, capture_latest_frame_bytes,
    capture_latest_frame_bytes_with_opts, capture_still, capture_still_bytes,
    capture_stream_latest,
};
pub use evidence::{StepEvidence, UiResolutionStrategy};
pub use execute::{
    RuntimeResult, execute_action_plan_with_progress, execute_action_plan_with_reliability,
};
pub use helper_budget::{HelperBudget, is_helper_budget_timeout};
pub use locator::{AxConstraint, AxQuality, Locator, score_ax_candidate};
pub use persist::{PersistedRunOutcome, run_persisted_action_plan};
pub use receipt::{ExecutionReceipt, ReceiptStep, build_receipt};
pub use semantic::{ResolvedTarget, SemanticError, UiTarget};
pub use verify::{StepVerification, VerificationStatus};
pub use vision_fallback::{
    VisionAxFailure, VisionHit, match_ocr_text, match_template_in_png,
    should_attempt_vision_for_error, should_prefer_vision_after_resolve,
};
