//! Local inference adapters (Ollama, LM Studio, OpenAI-compatible endpoints).

pub mod discovery;
pub mod openai_compatible;

pub use discovery::{LocalRuntimeDiscovery, discover_local_runtimes};
pub use openai_compatible::LocalCompatibleProvider;
