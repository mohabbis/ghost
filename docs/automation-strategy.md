# Automation strategy

## Prefer this order

1. **API / connector** — when the system has a scoped API
2. **Browser automation** — semantic selectors, not pixels
3. **Desktop automation** — later, when the UI has no API
4. **Vision** — last resort

## Trust rules (non-negotiable)

- AI proposes / interprets; deterministic code executes approved plans
- Sensitive actions are deny-by-default (`classifyStep` — not the model)
- Every run writes audit events; reversible work should be recoverable
- Credentials are scoped, revocable, least-privilege

## GTM wedge

Do not sell “platform.” Sell **one workflow**:

1. Pick a recurring, measurable job (e.g. “submit supplier order from email PDF”)
2. Implement it with the customer ($ implementation)
3. Prove hours saved + audit trail
4. Sell more workflows / seats in the same firm

## What not to build next

- More desktop Organizer polish as a growth bet
- Broad connector surface before recording → editable steps works
- Unsupervised “just do it” modes
