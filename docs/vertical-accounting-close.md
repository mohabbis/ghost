# Vertical spec — accounting & bookkeeping month-end close

Status: **product/eng spec for the first sellable vertical, not yet built.** It
is the worked example behind `docs/automation-strategy.md` ("go-to-market is
vertical; the runtime is horizontal"). It reuses the existing Organizer trust
pipeline and Action Plan runtime — it does **not** propose a new engine.

Read first: `docs/gtm-organizer.md` (the persona), `docs/organizer-planner.md`
and `docs/organizer-executor.md` (the pipeline this rides on),
`docs/filing-profiles.md` (the Finance filing profile that already exists).

## The buyer and the job

Buyer: the ops lead / senior bookkeeper at a 10–50 person accounting or
bookkeeping firm (the `gtm-organizer.md` persona, sharpened). Every month they
close the books for N clients. For each client the same ritual repeats:

1. Pull exports and source docs from portals/email into a messy `Downloads`
   folder — bank statements, invoices, receipts, CC statements, payroll reports.
2. Rename them to the firm's convention (`ClientName_2026-06_BankStatement.pdf`).
3. File each into the client's period folder
   (`/Clients/Acme/2026/06-June/`).
4. Reconcile — confirm every expected document arrived; flag what's missing.
5. Produce something a reviewing partner (or, later, an auditor) can sign:
   "here is exactly what was filed this period, and nothing was silently
   changed."

Today this is manual, slow, and — critically — **already audit-shaped in the
user's head**. They think in "what moved, can I prove it." That is why this
vertical, not a consumer one, is the wedge: Ghost's hash-chained receipt is a
feature they understand on first contact, not a developer nicety.

Cloud RPA is barred: they have been told client financial data cannot go through
a cloud tool. That bar is Ghost's moat (`business-model.md` #1).

## What already ships (reuse, don't rebuild)

| Need | Existing capability |
| --- | --- |
| Classify invoice/receipt/statement | `organizer/classifier.rs`, Finance filing profile (`filing/finance.rs`) |
| Period-aware naming/foldering | `filing/period.rs`, dated renaming, client-filing preset (`organizer/naming.rs`) |
| Preview every move before it happens | `organizer/planner.rs` (read-only plan) |
| Execute with no silent delete/overwrite, undo written first | `organizer/executor.rs`, `audit/undo_journal.rs` |
| Tamper-evident run seal + verify | `storage/executions.rs::verify_chain`, `audit/audit_log.rs::to_compliance_report` |
| Signed, PII-masked audit export | `commands/organizer.rs` (`organizer_export_audit`), `audit/pii.rs` |
| One unified reviewable plan | `action_plan/compile.rs` (`action_plan_from_zone`) |

The close vertical is mostly **configuration + one new reconciliation step +
packaging**, not new runtime.

## The close as an Action Plan

A "Close Pack" is a saved configuration that compiles (via
`action_plan_from_zone`) into the same reviewable `ActionPlan` the runtime
already executes. Proposed shape:

```text
Close Pack "Monthly Bookkeeping Close"
  period: 2026-06 (derived from run date, user-confirmable)
  clients: [Acme, Borealis, ...]           # each maps to a Zone
  expected-docs checklist per client:
    - BankStatement (>=1)
    - CreditCardStatement (>=1)
    - Payroll (>=1 if client.has_payroll)
    - Invoices (>=0)
  naming rule: {Client}_{YYYY-MM}_{DocType}[_{n}].{ext}
  destination: {ClientRoot}/{YYYY}/{MM-Month}/
```

Run flow (each stage is existing pipeline unless marked **NEW**):

```text
Select Close Pack + period
  -> Scan client Downloads/intake Zones            (scanner.rs)
  -> Classify + propose names/destinations          (classifier/naming/planner)
  -> NEW: Reconcile against expected-docs checklist  (see below)
  -> Review plan (moves + missing-doc flags)         (Action Plan review UI)
  -> Approve                                         (single-use token)
  -> Execute: move/rename, undo written first        (executor.rs)
  -> Seal receipt + audit chain                      (receipt.rs / executions.rs)
  -> NEW: Emit Close Report (per-client packet)      (see below)
  -> Undo available for the whole run                (organizer_undo)
```

### NEW capability 1 — reconciliation — **implemented (commandless core)**

A pure, read-only step over an expected-docs checklist + the documents already
classified as present. For each client it emits a per-requirement
`Present | Partial | Missing` status (with `found`/`required` counts and the
matched file names) plus any `unexpected` present documents. This is
deterministic (counts by classified `ReportKind` against declared minimums) —
**no AI in the decision path**, consistent with rule 1 (AI may propose
classification; the reconcile verdict is arithmetic).

Landed at `src-tauri/src/finance/close/reconcile.rs` (`CloseChecklist`,
`ExpectedDoc`, `PresentDoc`, `reconcile() -> ReconcileReport`), reusing
`filing::finance::ReportKind` as the doc-type taxonomy and `filing::Period`.
It touches: **nothing** — pure in/struct out, no files, OS input, network, or
secrets, and it never mutates. It stays **commandless**: a Tauri surface on top
must still carry its own module, risk class, policy check, approval, and
audit/undo behavior (see the command table below). Surplus documents (`found >
required`) are surfaced via the count for the human to resolve; the core never
deletes a "duplicate".

### NEW capability 2 — the Close Report (the sellable artifact)

After execution, package the sealed run into a per-client "close packet": the
receipt (`get_execution_receipt`), the reconciliation result, and the
PII-masked audit export, rendered to a signable PDF/HTML the reviewing partner
approves. This is the deliverable the firm is actually buying — proof of a clean
close. It reuses `audit_log.rs::to_compliance_report` and the existing masked
export; the only new work is the per-client packaging + render.

Touches: **files** (writes the report into the client folder, through the same
no-overwrite executor path), no network.

## Proposed command surface

Following `docs/command-registry.md` conventions — each needs a module, risk
class, policy check, approval, audit/undo, and a registry entry before it lands.

| Command | Risk | Files | Net | Notes |
| --- | --- | --- | --- | --- |
| `close_pack_list` / `_save` / `_delete` | low | config only | no | Pack CRUD; a Close Pack is a policy-pack superset, reuse `organizer_*_policy_pack` plumbing |
| `close_plan` | low (read-only) | read | no | Scan+classify+reconcile → reviewable `ActionPlan`; mutates nothing |
| `close_execute` | medium | write | no | Reuses `execute_action_plan`; undo-first, seals chain |
| `close_report_export` | low | write | no | Renders per-client close packet from the sealed receipt |

No command here introduces network, OS-input, or secret access. If a firm later
wants "pull the exports for me" (portal login/scrape), that is a **Routines**
job under its own grant and threat model — explicitly out of scope for v1, which
starts from files already on disk. Keep it out to protect the trust story.

## Trust invariants specific to this vertical

- Reconcile and report are **read-only / additive**; they never delete a
  "duplicate," only flag it for the human.
- The reconcile verdict is deterministic arithmetic, not an AI judgment call.
- A missing expected doc **blocks nothing** — it is surfaced in review; the human
  decides whether to proceed. Ghost does not fabricate or fetch the missing doc.
- The Close Report is derived from the *sealed* receipt, so it can never disagree
  with what actually executed (same discipline as `audit/pii.rs` masking export
  text without touching the hash-chained log).

## Pricing/packaging note (not a price commitment)

This is where per-seat B2B pricing gets justified: the firm pays per bookkeeper
seat, and the Close Report + team audit layer (`docs/scheduled-runs-and-team-audit.md`)
is what a partner or external auditor consumes. Do not publish a price here — the
owner has asked to hold pricing. This doc only establishes that the *value* being
priced is trustworthy close-proof, not "file moving."

## Smallest shippable slice

To validate the vertical with one real firm without building all of the above:

1. A **Finance-profile multi-client Zone setup** + the naming/destination
   convention (mostly config over existing capability).
2. `organizer/reconcile.rs` + surfacing `Present/Missing` in the existing plan
   review UI.
3. `close_report_export` producing one PDF packet from an already-sealed run.

That is one new pure module, one new export command, and UI surfacing — no new
runtime, no network, no autonomy. It proves "close-proof in minutes" to exactly
the buyer who is barred from cloud tools.
