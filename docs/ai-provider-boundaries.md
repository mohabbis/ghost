# AI Provider Boundaries

Ghost may eventually support natural-language planning inside the desktop app, but provider output must remain suggestion-only. It must not contain directly executable operations and must not bypass the trust pipeline.

## Boundary

MCP lets external AI clients use Ghost tools. The internal provider layer is separate and exists only for Ghost-owned UI flows.

```text
User instruction in Ghost UI
        |
        v
IntelligenceProvider
        |
        v
Validated suggestion schema
        |
        v
Ghost deterministic planner
        |
        v
Policy evaluation
        |
        v
Desktop approval
        |
        v
Execution -> Audit -> Undo
```

## Provider interface

Providers should implement a narrow interface such as:

```rust
pub trait IntelligenceProvider {
    async fn propose_plan(
        &self,
        request: PlanningRequest,
    ) -> Result<PlanningSuggestion, ProviderError>;
}
```

Potential implementations:

- `OpenAiProvider`;
- `AnthropicProvider`;
- `LocalModelProvider`;
- `DisabledProvider`.

Network-backed providers are not part of the default local-first product. They require explicit user configuration, documented data sharing, and scoped inputs.

## Suggestion schema

Provider responses should use constrained data structures:

```rust
pub struct PlanningSuggestion {
    pub objective: String,
    pub classifications: Vec<FileClassification>,
    pub proposed_rules: Vec<OrganizationRule>,
    pub uncertainty: Vec<UncertainItem>,
}
```

The deterministic planner, not the provider, creates executable operations.

## Redaction defaults

Send the minimum metadata needed for the task. For Organizer planning, prefer:

```json
{
  "name": "invoice-july.pdf",
  "extension": "pdf",
  "size": 84521,
  "created_at": "2026-07-08",
  "zone_relative_path": "invoice-july.pdf"
}
```

Do not send by default:

- file contents;
- absolute usernames or full paths;
- document text;
- hidden files;
- credentials or secrets;
- browser data;
- email contents;
- screenshots or screen contents.

## Module shape

**Partially built** — see `docs/ai-provider-architecture.md` for status.

```text
src-tauri/src/intelligence/
├── mod.rs
├── provider.rs      built — IntelligenceProvider trait
├── schema.rs        built — PlanningSuggestion (suggestion-only)
├── redaction.rs     built
├── registry.rs      built — disabled default
├── router.rs        built
├── disabled.rs      built
├── openai.rs        planned
├── anthropic.rs     planned
└── local/           planned
```

Provider adapters may produce suggestions only. They must not write files, move files, run shell commands, synthesize input, or call execution paths directly.
