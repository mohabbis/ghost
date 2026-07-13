//! Provider-neutral MCP server for external AI clients (planned).
//!
//! All MCP clients (Claude, Codex, Cursor, ChatGPT-compatible) share this surface.
//! See `docs/mcp-integration.md`.

pub mod approval;
pub mod errors;
pub mod execute;
pub mod handlers;
pub mod http;
pub mod pairing;
pub mod pending;
pub mod plan_hash;
#[cfg(feature = "experimental")]
pub mod relay;
pub mod server;
#[cfg(feature = "experimental")]
pub mod tls;
pub mod token_store;
pub mod tools;

pub use approval::{issue_approval_token, ApprovalTokenClaims, SignedApprovalToken};
pub use http::{
    http_server_status, localhost_http_running, run_localhost_http, start_http_server,
    stop_http_server, HttpServerOptions, HttpServerStatus,
};
pub use pairing::{disable_pairing, enable_pairing, pairing_is_required, pairing_status};
pub use plan_hash::hash_organizer_plan;
#[cfg(feature = "experimental")]
pub use relay::{relay_status, start_relay, stop_relay, RelayOptions, RelayStatus};
pub use server::run_stdio;
pub use tools::McpToolKind;
