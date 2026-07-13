//! Layer C: internal intelligence providers (suggestion-only, no execution).
//!
//! External AI clients use the MCP server (`crate::mcp`), not this module.

pub mod anthropic;
pub mod capability;
pub mod credentials;
pub mod disabled;
pub mod errors;
pub mod local;
pub mod openai;
pub mod parse;
pub mod prompt;
pub mod provider;
pub mod redaction;
pub mod registry;
pub mod router;
pub mod schema;
pub mod service;

pub use anthropic::AnthropicProvider;
pub use capability::{ProviderCapabilities, ProviderHealth, ProviderId};
pub use credentials::{CredentialProvider, CredentialStore};
pub use disabled::DisabledProvider;
pub use errors::ProviderError;
pub use local::{discover_local_runtimes, LocalRuntimeDiscovery};
pub use provider::IntelligenceProvider;
pub use registry::ProviderRegistry;
pub use router::{ProviderRouter, RoutingPolicy};
pub use schema::{PlanningRequest, PlanningSuggestion};
pub use service::{build_router, provider_id_from_config};
