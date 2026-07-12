# Local Models

Status: **planned** (architecture only)

## Principle

**Locality is not permission.** A local model has the same suggestion-only boundary as remote providers.

## Planned backends

| Backend | Adapter module |
|---|---|
| Ollama | `intelligence/local/ollama.rs` |
| LM Studio | `intelligence/local/lm_studio.rs` |
| OpenAI-compatible endpoint | `intelligence/local/openai_compatible.rs` |

## Discovery — **planned**

- Probe common localhost ports only
- Never scan LAN without explicit consent
- Warn before sending metadata to non-local endpoints
- Health check without mutation

## Configuration shape — **planned**

```rust
pub struct LocalProviderConfig {
    pub backend: LocalBackend,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub allow_network_lan: bool,
}
```

## Routing — **partially built**

`RoutingPolicy::local_only_for_sensitive_data` exists; enforcement when providers are implemented.

## Related

- `docs/ai-provider-architecture.md`
- `docs/data-redaction.md`
