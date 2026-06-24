//! Ghost Tauri command registry.
//!
//! Public re-exports keep the `commands::command_name` registration paths stable
//! while the implementations live in command groups with clearer product
//! boundaries.

pub mod auth;
pub mod core;
pub mod diagnostics;
pub mod experimental;

pub use auth::*;
pub use core::*;
pub use diagnostics::*;
pub use experimental::*;
