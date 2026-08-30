# Product direction

Ghost is the **governed execution / trust runtime for AI agents** — not another
autonomous agent.

```text
Agent (propose) → Ghost (approve · execute · verify · audit)
```

Teach Ghost a business workflow once. Agents (Claude, Cursor, Codex, …) may
propose or start runs through Ghost’s tool surface; **humans approve sensitive
steps** in Ghost; deterministic code executes; outcomes are verified; every run
leaves a hash-chained audit log.

```text
Capture → Review → Approve → Execute → Verify → Recover
```

Prefer, in order: **APIs → browser automation → desktop → vision last.**

Working name **Ghost** will likely be renamed (generic / hard to brand for
trust). Strategy does not wait on naming.

## Promise

> Agents propose. Ghost runs the job with a human gate on anything sensitive,
> verifies the outcome, and leaves a record of exactly what changed.

## Commercial wedge

Sell **trusted execution** into ops teams that already use AI assistants:

- wholesale / distribution order ops
- property management admin
- bookkeeping / month-end document flow
- logistics / recruiting / healthcare-admin back office

First money: **implementation** of one painful, measurable, reversible workflow
(+ seats). Distribution also rides agents via **MCP / HTTP tools** (see
`cloud/docs/AGENT_PLUGIN.md`).

## Competitive position

Not uncontested. `competitive-landscape.md` names one head-on competitor
(**Skyvern** — AGPL, SOC 2 Type II, HIPAA, named customers in bookkeeping and
healthcare admin) and records that the browser-infrastructure layer below Ghost
has started shipping human-in-the-loop as a platform primitive. Ghost's surviving
differentiator is narrow and specific: the gate is a **pure function over a typed
step schema**, the audit chain is **tamper-evident** rather than merely written,
and verification plus compensation are part of the pipeline. Read that document
before making a positioning claim here.

## Non-goals

- Not a chatbot or “AI coworker” that acts unsupervised
- Not competing on more autonomy than Claude/Cursor
- Not a Zapier/Make clone (connectors without a trust pipeline)
- Not a desktop Organizer utility as the commercial product
- Not vision-first RPA

## Active vs legacy

| Surface | Status |
|---|---|
| `cloud/` SaaS + agent plugin API/MCP | **Active** — build and sell this |
| Root Rust/Tauri desktop | **Legacy** — retained; not the roadmap |

## Authoritative docs

`cloud/README.md` · `cloud/docs/PHASE_1_PLAN.md` · `cloud/docs/CURSOR_HANDOFF.md` ·
`cloud/docs/AGENT_PLUGIN.md` · `AGENTS.md` · this file · `business-model.md` ·
`competitive-landscape.md`
