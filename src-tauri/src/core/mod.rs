//! Core module for platform-agnostic input handling.

pub mod ai;
pub mod cloud;
pub mod events;
pub mod traits;
pub mod llm;
pub mod vision;
pub mod wait;
pub mod security;

pub use ai::*;
pub use cloud::*;
pub use events::*;
pub use traits::*;
pub use llm::*;
pub use vision::*;
pub use wait::*;
pub use security::*;
