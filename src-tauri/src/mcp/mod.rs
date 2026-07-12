//! Provider-neutral MCP server for external AI clients (planned).
//!
//! All MCP clients (Claude, Codex, Cursor, ChatGPT-compatible) share this surface.
//! See `docs/mcp-integration.md`.

pub mod approval;
pub mod errors;
pub mod tools;

pub use approval::ApprovalTokenClaims;
pub use tools::McpToolKind;
