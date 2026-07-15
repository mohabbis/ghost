# AI Provider Architecture

Status key: **built** | **partially built** | **experimental** | **planned**

## Boundary

Ghost separates three integration layers (see `docs/integrations-roadmap.md`):

| Layer | Purpose | Module |
|---|---|---|
| A — Identity | Who signed in | `identity/` |
| B — Business systems | Fabric, Power BI, Google Cloud | `integrations/` |
| C — Intelligence | Suggestion-only models + MCP clients | `intelligence/`, `mcp/` |

Internal intelligence providers (Layer C, Ghost UI) must never execute. External AI clients use the MCP server — not separate per-vendor execution paths.

## Pipeline (non-negotiable)

```text
Intent -> Suggestion -> Deterministic plan -> Policy -> User approval -> Execution -> Audit -> Undo
```

## Module layout — **partially built**

```text
src-tauri/src/intelligence/
├── mod.rs           built — module boundary
├── provider.rs      built — IntelligenceProvider trait
├── capability.rs    built — ProviderId, capabilities, health
├── schema.rs        built — PlanningRequest, PlanningSuggestion
├── redaction.rs     built — shared metadata minimization
├── registry.rs      built — provider registry (disabled default)
├── router.rs        built — user-controlled routing policy
├── disabled.rs      built — default no-op provider
├── openai.rs        built — OpenAiProvider
├── anthropic.rs     built — AnthropicProvider
└── local/           planned — Ollama, LM Studio, OpenAI-compatible
```

## IntelligenceProvider — **built**

Providers implement `propose_plan` only. Output is validated against `PlanningSuggestion` and `suggestion_is_safe()` heuristics before any planner consumes it.

## Routing — **built (policy only)**

`ProviderRouter` + `RoutingPolicy`:

- user chooses default provider;
- no silent fallback unless `allow_fallback` is explicitly enabled (default: false);
- `local_only_for_sensitive_data` reserved for future enforcement.

Automatic “best model” routing is **planned**, not built.

## Related docs

- `docs/ai-provider-boundaries.md` — original boundary spec
- `docs/data-redaction.md` — shared redaction rules
- `docs/mcp-integration.md` — external clients (separate from internal providers)
