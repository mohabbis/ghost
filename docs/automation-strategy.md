# Automation strategy

## Prefer this order

1. **API / connector** — when the system has a scoped API
2. **Browser automation** — semantic selectors, not pixels
3. **Desktop automation** — later, when the UI has no API
4. **Vision** — last resort

## Trust rules (non-negotiable)

- AI proposes / interprets; deterministic code executes approved plans
- Sensitive actions are deny-by-default (`classifyStep` — not the model)
- **Agents never approve** — human-only in Ghost UI (`cloud/docs/AGENT_PLUGIN.md`)
- Every run writes audit events; reversible work should be recoverable
- Credentials are scoped, revocable, least-privilege

## Product shape

Ghost is the **trust runtime agents plug into**:

```text
Agent (propose) → Ghost (approve · execute · verify · audit)
```

Build execution quality first; agent distribution (MCP/HTTP tools) rides that
engine. Do not invert: an agent wrapper without a real gate is not the product.

## GTM wedge

Do not sell “platform.” Sell **one workflow**:

1. Pick a recurring, measurable job (e.g. “submit supplier order from email PDF”)
2. Implement it with the customer ($ implementation)
3. Prove hours saved + audit trail
4. Optionally expose start/status via agent tools; approval stays in Ghost
5. Sell more workflows / seats in the same firm

## What not to build next

- More desktop Organizer polish as a growth bet
- Broad connector surface before recording → editable steps works
- Unsupervised “just do it” modes or agent self-approval
- Remote ChatGPT connector theater before the local MCP/API loop is solid
