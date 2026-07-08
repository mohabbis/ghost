//! Core module for platform-agnostic input handling.

pub mod ai;
pub mod cloud;
pub mod compress;
pub mod compression;
pub mod dry_run;
pub mod events;
pub mod execution;
pub mod knowledge;
pub mod llm;
pub mod ocr;
pub mod replay_support;
pub mod security;
pub mod traits;
pub mod vision;
pub mod wait;
pub mod workflow_schema;

pub use ai::*;
pub use cloud::*;
pub use events::*;
pub use execution::*;
pub use knowledge::*;
pub use llm::*;
pub use ocr::*;
pub use replay_support::*;
pub use security::*;
pub use traits::*;
pub use vision::*;
pub use wait::*;
pub use workflow_schema::*;

pub mod guard;
