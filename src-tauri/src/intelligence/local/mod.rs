//! Local inference adapters (Ollama, LM Studio, OpenAI-compatible endpoints).

pub mod discovery;
pub mod openai_compatible;

pub use discovery::{discover_local_runtimes, LocalRuntimeDiscovery};
pub use openai_compatible::LocalCompatibleProvider;
