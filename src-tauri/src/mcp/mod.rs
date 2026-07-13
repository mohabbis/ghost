//! Provider-neutral MCP server for external AI clients (planned).
//!
//! All MCP clients (Claude, Codex, Cursor, ChatGPT-compatible) share this surface.
//! See `docs/mcp-integration.md`.

pub mod approval;
pub mod errors;
pub mod handlers;
pub mod server;
pub mod tools;

pub use approval::{issue_approval_token, ApprovalTokenClaims, SignedApprovalToken};
pub use server::run_stdio;
pub use tools::McpToolKind;
