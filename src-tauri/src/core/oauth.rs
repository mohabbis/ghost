//! OAuth 2.0 + PKCE sign-in — re-exported from `identity::oauth` for existing imports.
//!
//! New code should import from `crate::identity` directly.

pub use crate::identity::oauth::*;

// Backward-compatible alias used by `commands/account.rs`.
pub type Provider = OAuthProvider;
