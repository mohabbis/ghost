//! Platform-specific backend implementations.
//! Re-exports the appropriate backend for the target OS.

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
#[cfg(target_os = "windows")]
pub use windows::*;
