//! Ghost Tauri commands - platform-agnostic IPC handlers.
//!
//! Keep this file as the public command registry. Implementations live in
//! smaller modules so stable core behavior does not get buried under experiment
//! drift, humanity's favorite maintenance strategy for some reason.

mod auth;
mod compression;
mod core;
mod diagnostics;
#[cfg(feature = "experimental")]
mod experimental;
mod organizer;
mod updates;

pub use auth::*;
pub use compression::*;
pub use core::*;
pub use diagnostics::*;
#[cfg(feature = "experimental")]
pub use experimental::*;
pub use organizer::*;
pub use updates::*;
