# Product Direction — Ghost is a cloud AI operator

> **Current direction.** This supersedes the earlier "Organizer-first, local-first
> desktop" direction. The active product is the cloud SaaS in `cloud/`. The
> Rust/Tauri desktop app at the repo root is legacy (retained in-tree). See
> `cloud/README.md`, `cloud/docs/PHASE_1_PLAN.md`, `cloud/docs/CURSOR_HANDOFF.md`.

## Decision

Ghost is an **AI operator**: it learns a business workflow once, then executes it
across the software a company already uses — combining APIs, browser automation,
and (later) desktop automation — with human approval on sensitive actions,
verification of outcomes, and a full audit log.

The core promise:

> Teach Ghost a workflow once. Ghost executes it reliably, adapts when the
> interface changes, verifies the outcome, and involves a human only when
> necessary — leaving a record of exactly what it changed.

Ghost is **not** a chatbot, a no-code automation builder, a macro recorder, or a
generic AI assistant. It is an **execution platform**. The differentiator is
taught-then-trusted execution: demonstrate a real process once; Ghost executes,
adapts, verifies, and escalates exceptions.

## Why this shape

- **Real pain, ugly systems.** Operations-heavy businesses move information by
  hand between disconnected tools (email, PDFs, Excel, ERPs, CRMs, portals, legacy
  desktop apps). That fragmentation is exactly where Ghost has value.
- **Hybrid execution is the moat.** Prefer **APIs → browser → desktop → vision
  fallback**. Many businesses run software without clean integrations; Ghost
  bridges them without a rip-and-replace.
- **Trust as the wedge into regulated ops.** Approval gates, verification, and a
  tamper-evident audit log make automation an auditor will accept — a claim generic
  agents can't make.

## Target customer (first)

Operations-heavy SMBs where employees repeatedly move information between systems:
wholesale distributors, property managers, accounting/bookkeeping firms, logistics,
recruiting, financial-operations and healthcare-administration teams. Start with
workflows that are **repetitive, measurable, and reversible** (reporting,
reconciliation, customer-ops, document processing, onboarding).

## Product loop

```text
Capture -> Review -> Approve -> Execute -> Verify -> Audit -> Recover
```

Required properties:

1. AI proposes/interprets; deterministic code executes approved plans.
2. Sensitive actions (send, pay, delete, submit) are deny-by-default and gated on
   human approval — decided by a deterministic classifier, not the model.
3. Every step's outcome is verified; a run isn't done because a button was clicked.
4. Every run and mutation is written to a hash-chained audit log.
5. Reversible actions are recoverable; credentials are scoped, least-privilege,
   revocable, and never captured into logs/screenshots/prompts.

## What to build (order)

1. **Execution engine** — approval-gated replay across browser + API, with
   verification, screenshots, audit. (Phase 1 — built.)
2. **Recording → editable workflow** — capture a demonstration, compile to typed
   reviewable steps. (Phase 2.)
3. **Connectors** — Gmail/Outlook, Salesforce, HubSpot, QuickBooks, storage, etc.,
   with scoped credentials, through the same pipeline.
4. **Intelligence** — suggestion-only reasoning (intent, extraction, summaries,
   next-action under ambiguity).

See `cloud/docs/PHASE_1_PLAN.md` for the detailed plan and `docs/business-model.md`
for pricing/packaging.

## Positioning language

Use: *AI operator*, *execution platform*, *teach it once*, *approval-gated*,
*verified*, *auditable*. Avoid: *chatbot*, *no-code builder*, *macro recorder*,
*fully autonomous agent*.
