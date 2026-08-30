# Business model

Ghost sells **governed workflow execution** — the trust layer AI agents plug
into — not AI novelty and not “more autonomy.”

```text
Agent (propose) → Ghost (approve · execute · verify · audit)
```

## How money comes in (order of realism)

1. **Implementation** — $2,500–15,000 to stand up one high-value workflow for a
   customer (configure, approve-gates, verify, hand off).
2. **Seats / team** — recurring after the workflow is in production.
3. **Agent distribution** — MCP / HTTP tools so Claude/Cursor/Codex users run
   work *through* Ghost (still billed as Ghost seats/runs, not as an agent
   subscription).
4. **Self-serve** — later, once the product earns it. Do not lead with free.

| Tier | Price | For |
|---|---|---|
| Individual | $29–49 / mo | One operator, their own workflows |
| Professional | $99–199 / user / mo | Connector-backed multi-step work |
| Team | $500–2,000 / mo | Runs, integrations, governance |
| Implementation | $2.5k–15k one-time | First production workflow |

Today this is also a portfolio-grade open build — no fake customer logos. Pricing
is the intended commercial shape once there is a paid pilot, not a claim that
those tiers are live. Working name may change.

## Moat (engineered, not assumed)

- **Trust runtime** — agents cannot approve sensitive steps; Ghost owns the
  gate, verify, and audit. Generic agents become clients, not competitors.
  *Qualified:* browser-automation competitors now sell human-in-the-loop as an
  enterprise feature, and the infrastructure layer ships it as a primitive. What
  is defensible is the narrower claim — a deterministic, non-model gate over a
  typed schema plus a tamper-evident chain. See `competitive-landscape.md`.
- **Hybrid execution** — APIs → browser → desktop → vision. Works where clean
  integrations don't exist.
- **Accumulated workflow IP** — each configured, approved workflow raises
  switching cost.

## What we will not do for revenue

- Skip approval on send/pay/delete/submit to “increase automation rate”
- Let an agent approve its own gated actions
- Hold customer data hostage
- Market the legacy desktop Organizer as the product we sell

## Near-term commercial proof

Land **one** paying (or paid-pilot) workflow on the cloud engine, callable from
an agent tool surface: propose/start → human approve → execute → verify → audit.
See `cloud/docs/AGENT_PLUGIN.md`.
