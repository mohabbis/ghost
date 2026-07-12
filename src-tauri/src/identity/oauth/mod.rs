//! OAuth 2.0 + PKCE helpers (public client, no embedded secret).

pub mod callback;
pub mod flow;
pub mod pkce;
pub mod provider;

pub use flow::{run_sign_in_flow, SignInResult};
pub use provider::OAuthProvider;
