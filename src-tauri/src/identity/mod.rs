//! Layer A: account identity, integration grants, and encrypted token storage.
//!
//! Local vault password encryption remains in `auth.rs`. This module handles
//! OAuth-linked account identity and Layer B integration grants separately.

pub mod errors;
pub mod oauth;
pub mod store;
pub mod types;

pub use errors::IntegrationError;
pub use oauth::{run_sign_in_flow, OAuthProvider};
pub use store::IdentityStore;
pub use types::{
    AccountIdentity, IdentityBundle, IdentityProvider, IntegrationGrant, IntegrationKind,
    ResourceScope, TokenMaterial, TokenRecord,
};
