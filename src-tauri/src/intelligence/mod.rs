//! Layer C: internal intelligence providers (suggestion-only, no execution).
//!
//! External AI clients use the MCP server (`crate::mcp`), not this module.

pub mod capability;
pub mod disabled;
pub mod errors;
pub mod provider;
pub mod redaction;
pub mod registry;
pub mod router;
pub mod schema;

pub use capability::{ProviderCapabilities, ProviderHealth, ProviderId};
pub use disabled::DisabledProvider;
pub use errors::ProviderError;
pub use provider::IntelligenceProvider;
pub use registry::ProviderRegistry;
pub use router::{ProviderRouter, RoutingPolicy};
pub use schema::{PlanningRequest, PlanningSuggestion};
