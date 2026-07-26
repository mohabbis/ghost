# Architecture — scheduled runs & the team audit layer

Status: **architecture proposal, not built.** This is the highest-leverage step
from one-shot desktop tool to per-seat subscription, per
`docs/automation-strategy.md`. It describes two layers built on the existing
Action Plan runtime without weakening any trust invariant:

1. **Scheduled / triggered runs** — recurring execution that still resolves to an
   approval and still seals a receipt.
2. **Team audit layer** — aggregating sealed receipts across seats for a
   compliance officer / reviewing partner, under scoped opt-in sync.

Read first: `docs/approval-tokens.md`, `docs/organizer-executor.md` ("Crash
recovery"), `docs/business-model.md` (why the team layer is where the business
lives), and `docs/integrations-roadmap.md` (the grant model any sync must reuse).

## Problem statement

Local-first gives zero switching cost and no usage metering for free
(`business-model.md`, "the hard truth"). A single-user tool that runs once when
you click it is easy to leave. The two layers here manufacture the missing
recurring value: a habit the firm depends on weekly, and an audit record their
reviewer can't recreate elsewhere — without ever putting client files in the
cloud.

## Layer 1 — scheduled / triggered runs

### The hard constraint

`CLAUDE.md` rule 1 and the whole trust brand say deterministic code executes
**only approved plans**, and nothing mutates without approval. A naive scheduler
that mutates at 2am unattended breaks that. So the design question is not "how do
we run on a schedule" — it's **"how does approval work when no human is at the
keyboard at run time."**

### Two approval modes

**Mode A — Scheduled propose, human approves (default, always available).**
The schedule fires only the *read-only* half: scan + classify + plan +
reconcile. It produces a reviewable `ActionPlan` and a notification ("June close
plan ready — 142 files, 2 clients missing a bank statement"). Nothing mutates
until the human opens it and approves. This is just cron-triggering
`close_plan` / `action_plan_from_zone`; it introduces **no new trust surface**.
Ship this first.

**Mode B — Pre-authorized policy-scoped run (opt-in, bounded).** For firms that
want true unattended execution, the human pre-approves a *bounded policy scope*,
not a specific plan. At run time the runtime executes only steps that fall inside
that scope and seals a receipt; anything outside it (a new destination, a
higher-risk capability, a conflict) is held for interactive approval and
executed on none of it.

Bounding a Mode B grant (all enforced by `policy/engine.rs`, deny-by-default):

- capability allowlist (e.g. `fs_move`, `fs_rename` only — never `fs_delete`);
- Zone/destination allowlist (only the declared client roots);
- per-run caps (max files, max total bytes moved);
- risk ceiling (auto-execute only `RiskLevel::Low`; anything higher → held);
- expiry (a grant is time-boxed and re-consented, like `IntegrationGrant`);
- kill switch: any conflict, any `Deny`, any out-of-scope step → abort the run,
  execute nothing further, surface for interactive review.

A Mode B grant is **not** a standing license to mutate silently — it is a
narrow, revocable, expiring, receipt-sealing authorization. That distinction is
the entire reason this stays inside the trust model.

### Reusing the approval-token machinery

Interactive approval already issues a single-use, plan-hash-bound token
(`mcp/plan_hash.rs`, `mcp/token_store.rs`, `docs/approval-tokens.md`). Mode B
generalizes this to a **scope-bound grant token**: bound to a policy scope hash
rather than one plan hash, single-use per scheduled firing, and validated by the
same `execute_action_plan` path. No mutation path bypasses token validation —
the scheduler is just another token issuer, held to the same rules.

### Scheduler mechanics

- A local, on-device scheduler (OS-native: launchd on macOS, Task Scheduler on
  Windows) triggers the Ghost binary in a headless plan/execute mode. No server,
  no cloud cron — consistent with local-first.
- Triggers v1: time-based (monthly/weekly/daily). Later: folder-watch ("when N
  new files land in an intake Zone, propose a plan"). Folder-watch is still
  propose-only under Mode A.
- **Crash/interruption reuses what exists.** Scheduled runs write the same WAL
  as interactive ones (`storage/executions.rs::begin_execution` /
  `update_execution_progress` / `finish_execution`); an interrupted 2am run
  leaves a recoverable `finished = 0` record surfaced on next open
  (`organizer_check_unfinished_run`), exactly like today. Replay has the mirror
  (`replay_check_unfinished_run`). Nothing new to invent for durability.

### Proposed command surface

| Command | Risk | Notes |
| --- | --- | --- |
| `schedule_list` / `_create` / `_delete` / `_pause` | low | Config only; registers/removes the OS-native trigger |
| `schedule_grant_create` / `_revoke` | medium | Creates/revokes a Mode B scope grant; revoke is instant + audited |
| `schedule_run_now` | (delegates) | Fires a scheduled plan immediately; same approval mode as configured |

Every scheduled execution flows through the unchanged `execute_action_plan`
path — same policy re-check per action, undo-first, receipt seal. The scheduler
adds *triggering and scoped authorization*, never a second execution path.

## Layer 2 — the team audit layer

### What it is

Multiple seats in a firm; each seat runs Ghost locally and seals receipts
(`runtime/receipt.rs`) + a per-machine hash chain
(`storage/executions.rs::verify_chain`). The team layer lets a compliance
officer / reviewing partner see, across seats: what ran, what moved, which rule
fired, whether each chain verifies intact, and what's still unreconciled — the
firm-wide "prove the close" view.

This is the layer that, per `business-model.md`, converts "nice tool" into a
per-seat business and creates an audit record competitors can't recreate.

### The privacy line (non-negotiable)

- **Files never sync.** Client documents stay on each machine, encrypted at rest.
  The privacy defaults in `CLAUDE.md` ("no cloud-first storage for
  workflow/organizer data") are absolute.
- **Only receipts, chain-verification results, reconciliation summaries, and
  policy packs sync** — and only under an explicit, scoped, revocable
  `IntegrationGrant` (`identity/` + the exact pattern the Power BI export already
  uses in `integrations/`). A seat opts in; sync is disclosed; it can be revoked.
- Synced receipts are **PII-masked** before they leave the machine, reusing
  `audit/pii.rs::mask` (the same masking `organizer_export_audit` and the Power
  BI payload already apply) — never the raw hash-chained log.

### Two deployment options for the aggregation point

1. **Bring-your-own-store (most on-brand).** Receipts export to a destination the
   firm already controls — a shared drive, or the Power BI dataset the export
   pipeline already builds (`integrations/microsoft/power_bi/export.rs`). Ghost
   adds the "team dashboard" as a *view over exported receipts*, no Ghost-run
   server. Lowest trust cost; ship this first.
2. **Optional hosted team backend (later, opt-in).** A thin server that stores
   only masked receipts + chain proofs for cross-seat rollup. This is the only
   place Ghost would run infrastructure, and it must be justified against the
   counter-positioning moat before building — a hosted store is exactly what the
   "your data never leaves your machine" pitch is selling *against*. If built:
   receipts only, never files; end-to-end the firm's own tenant; deletable.

### Chain verification across seats

Each seat's chain is independently verifiable (`verify_chain` →
`intact`/`sealed_count`/`first_break`). The team view aggregates these:
green if every seat's chain verifies and no gaps, flagged with the exact
`first_break` if any seat's chain is broken. A reviewing partner reads one
firm-wide status instead of trusting a spreadsheet. That is the compliance
artifact the firm can't reproduce with generic tools — the durable moat.

### Proposed command surface

| Command | Risk | Net | Notes |
| --- | --- | --- | --- |
| `team_sync_grant` / `_revoke` | medium | yes (scoped) | Opt-in `IntegrationGrant` for receipt sync; revoke is instant |
| `team_export_receipts` | low | yes | Pushes masked receipts + chain proofs to the chosen store; reuses Power BI export discipline |
| `team_verify_all` | low | no | Local rollup of each seat's `verify_chain` for the dashboard view |

`team_export_receipts` is the only networked command here, and it re-derives its
payload from local sealed history server-side of the frontend (never trusting a
frontend snapshot) — the same rule `power_bi_push_audit_export` already follows.

## Build order

1. **Mode A scheduled propose** (cron → read-only plan + notification). No new
   trust surface; immediate "it's ready every month-end" habit value.
2. **Team dashboard over exported receipts** (option 1, bring-your-own-store).
   Turns sealed receipts into the firm-wide close-proof view. This is the
   per-seat pricing justification.
3. **Mode B pre-authorized scoped runs.** Only after 1–2 prove the trust model
   holds in the field; this is the highest-trust-cost feature, so it ships last
   and stays opt-in and bounded.
4. **Hosted team backend** — only if bring-your-own-store demonstrably isn't
   enough, and only receipts-not-files, weighed against the counter-positioning
   moat first.

## Invariants this design must preserve

- No mutation without a validated token — the scheduler is another issuer, not a
  bypass.
- Mode B is bounded, expiring, revocable, receipt-sealing — never a standing
  silent-mutate license.
- Files never leave the machine; only masked receipts / proofs / policy packs do,
  under a scoped opt-in grant.
- Scheduled runs reuse the existing WAL/crash-recovery path unchanged.
- Every new command carries module + risk class + policy + approval + audit/undo
  + a `docs/command-registry.md` entry.
