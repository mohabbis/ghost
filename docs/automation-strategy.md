# Automation strategy — what Ghost sells beyond "file organizer"

Status: **strategy / planning doc, not a build order.** It reconciles two things
that look like they conflict but don't:

- the product guardrails in `CLAUDE.md` / `docs/product-direction.md` ("one
  problem, one customer, one price"; do **not** market Ghost as "workflow
  automation" or an "OS"), and
- the owner's intent to grow Ghost into a product companies pay for — not a
  single-user desktop utility.

Read `docs/business-model.md` (the moat and money model) and
`docs/gtm-organizer.md` (the beachhead persona) first. This doc sits on top of
both and answers one question: **what is the sellable thing under Ghost
Organizer, and how do we expand toward it without breaking the wedge?**

## The reframe (and why it isn't a pivot)

"Workflow automation" is a red ocean — Zapier, Make, n8n, UiPath, Power
Automate, Copilot Studio. Ghost cannot win there on feature count, and marketing
Ghost as generic automation invites exactly the comparison it loses. The
guardrails in `CLAUDE.md` are right to forbid that framing.

But Ghost is also genuinely more than a file organizer, and the code already
proves it. The Ghost 2.0 **Action Plan runtime** (`src-tauri/src/action_plan/`
+ `src-tauri/src/runtime/`) is a general trustworthy-execution engine:

```text
Capture -> Review -> Approve -> Execute -> Verify -> Recover
```

It already runs over **two** action families through one policy engine — file
steps (`runtime/fs.rs`, the Organizer pipeline) and semantic-UI steps
(`runtime/semantic.rs`/`ui.rs`, Routines) — and seals a signed execution receipt
(`runtime/receipt.rs`) with an undo path. Organizer is the first *skin* on that
engine, not the engine itself.

So the reframe is:

> **The product is auditable, approval-gated, reversible execution on the user's
> own machine. Ghost Organizer is the beachhead expression of it. We expand
> along the trust/audit moat, not toward feature parity with cloud automation.**

That is not a pivot away from the wedge — it is naming the platform the wedge is
already standing on. Every guardrail in `CLAUDE.md` survives intact: AI proposes,
deterministic code executes approved plans, deny-by-default, no silent
delete/overwrite, local-first, audit + undo on every mutation.

## What we sell (and to whom)

Not "automation." A **playbook** to a buyer who already feels a specific,
recurring, audit-sensitive pain — the ops-lead persona from `gtm-organizer.md`,
generalized one notch.

The pitch to a company, in one sentence each:

- **"Automation your auditor will accept."** Every run seals a hash-chained,
  signed receipt of exactly what changed (`storage/executions.rs::verify_chain`,
  `audit/audit_log.rs` compliance report). Cloud RPA cannot make this claim
  credibly because the trail lives on someone else's server.
- **"It never leaves the machine."** For client financials, PHI, and privileged
  legal files, cloud automation is a compliance project before it is a tool.
  Local-first is the unfair advantage here, not a limitation
  (`business-model.md`, moat #1 counter-positioning).
- **"It can always undo."** Deny-by-default + undo journal means a bad run is a
  reversal, not an incident.
- **"A human approves before anything mutates."** Sellable *because* it is not
  fully autonomous — the single-use, plan-hash-bound approval token
  (`mcp/plan_hash.rs`, `docs/approval-tokens.md`) is the product, not a footnote.

### The verticals (go-to-market is vertical; the runtime is horizontal)

Ranked by distance from what already ships. Each is a **playbook pack**, not a
new product surface — see `docs/vertical-accounting-close.md` for the worked
example.

1. **Accounting / bookkeeping — month-end close & client filing.** Closest to
   today's Organizer. The buyer already thinks "what moved, can I show it."
   Detailed spec: `docs/vertical-accounting-close.md`.
2. **Legal / paralegal — matter intake & filing.** Bates-style renaming,
   privilege-review packet assembly, redaction (`audit/pii.rs` already redacts
   on export). "Local-only" is the whole sale.
3. **Healthcare admin / medical billing.** HIPAA makes cloud RPA a burden;
   local + audited + reversible claims/EOB/record filing. On-device OCR + ID
   parsing already ship (`core/ocr.rs`, `core/id_scan.rs`).
4. **Legacy-system data entry (Routines / Guard Desk lineage).** Desktop apps
   with no API; record → review → approve → replay with semantic resolution and
   coordinate fallback. This is the UiPath job for firms that can't afford or
   can't trust UiPath.
5. **Insurance / claims intake.** OCR a document → fill a legacy form →
   approval-gate the submission. Same runtime, different pack.

Pick **one** to land (accounting). Do not message five at once — that reads as a
horizontal platform and forfeits the vertical-depth moat.

## What turns a tool into a subscription

A pure single-user utility is, per `business-model.md`, "a nice tool with a weak
business." The recurring revenue lives in three engineered layers, in build
order:

1. **Scheduled / triggered runs** — "run the close pack every month-end,"
   keeping the trust model (human approval each run, or a pre-approved
   policy-scoped run that still seals a receipt). Highest-leverage step from
   one-shot tool → SaaS. Architecture: `docs/scheduled-runs-and-team-audit.md`.
2. **Team / compliance audit layer** — per-seat, with aggregated receipts a
   compliance officer reviews. This is what justifies per-seat B2B pricing over
   a one-time license. Same doc.
3. **Playbook marketplace** — generalize `organizer_export_policy_pack` /
   `_import_policy_pack` into shareable, verified packs (the Raycast Store /
   VS Code extensions pattern from `business-model.md` moat #4). A network effect
   bolted onto a local app.

## Guardrails this strategy must not cross

These keep the strategy inside `CLAUDE.md` rather than around it:

- **No node-graph "automation builder" UI.** That is the Zapier trap — it invites
  the comparison we lose and dilutes the "a human reviews a *plan*" story that is
  the actual differentiator. Plans are reviewed, not wired.
- **No new marketing category words.** Never "workflow automation platform,"
  "operating system," or "autonomous agent" in copy (rule 10). The words are
  "auditable," "local," "reversible," "approval-gated."
- **No autonomy creep.** Scheduled runs still resolve to an approval — either
  interactive or a pre-authorized, policy-scoped, receipt-sealing grant. A
  schedule never becomes a standing license to mutate silently.
- **No cloud storage of workflow/organizer data.** Team/audit sync, if built,
  syncs *receipts and policy packs* under scoped opt-in grants
  (`identity/` + `IntegrationGrant`), never the user's files.
- **Every new command still carries** a module, risk class, policy check,
  approval step, and audit/undo behavior, plus a `docs/command-registry.md`
  entry. The verticals do not get a fast lane around the trust pipeline.

## One-paragraph summary for a pitch deck

Ghost is auditable desktop automation for people who handle sensitive files and
can't use cloud tools. It scans, proposes an exact plan, waits for approval, then
moves/renames/fills — sealing a tamper-evident receipt and an undo path for every
run, entirely on the user's machine. It lands with accountants and bookkeepers
doing month-end close, and expands to legal, healthcare, and legacy data entry as
verified playbook packs — sold per seat, with a team audit layer their reviewers
and regulators accept.
