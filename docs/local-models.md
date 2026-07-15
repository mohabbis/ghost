# Local Models

Status: **built** (experimental — gated behind `--features experimental`)

## Principle

**Locality is not permission.** A local model has the same suggestion-only boundary as remote providers.

## Backends

| Backend | Config value | Default endpoint |
|---|---|---|
| Ollama | `ollama` | `http://127.0.0.1:11434/v1` |
| LM Studio | `lm_studio` | `http://127.0.0.1:1234/v1` |
| OpenAI-compatible | `openai_compatible` | user-supplied |

All three share `intelligence/local/openai_compatible.rs` (`LocalCompatibleProvider`).

## Discovery — **built**

`intelligence_discover_local` probes localhost only:

- Ollama: `127.0.0.1:11434/api/tags`
- LM Studio: `127.0.0.1:1234/v1/models`

Never scans LAN without `allow_network_lan` in config.

## Configuration

Stored in `GhostConfig.intelligence.local` (`config.rs`):

- `backend`, `base_url`, `model`, `api_key` (optional), `timeout_seconds`, `allow_network_lan`, `max_input_bytes`

Default intelligence provider values: `local_ollama`, `local_lm_studio`, `local_openai_compatible`.

Non-localhost endpoints are rejected unless `allow_network_lan` is enabled.

## Settings UI — **built**

Experimental Settings panel: backend/base URL/model fields, LAN toggle, discover button, health test.

## Routing — **built**

`RoutingPolicy::local_only_for_sensitive_data` is enforced in `ProviderRouter` when selecting providers for sensitive metadata.

## Related

- `docs/ai-provider-architecture.md`
- `docs/data-redaction.md`
