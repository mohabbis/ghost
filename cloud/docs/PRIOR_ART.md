# Prior art: what Ghost takes from the OSS workflow orchestrators

Ghost is not the first system to execute multi-step workflows reliably. It *is*
unusual in what it optimizes for — approval-gated, verified, auditable execution
against software that was never designed to be automated. This document surveys the
mature open-source orchestrators, records what Ghost borrows from each, and — more
importantly — records what Ghost deliberately refuses and why.

The refusals matter more than the borrowings. Ghost's whole value is a narrow,
trustworthy pipeline. Most of what these platforms offer would widen it.

## The one-line summary

| Platform | Its core idea | Ghost's verdict |
|---|---|---|
| [Temporal](https://docs.temporal.io/workflow-execution) | Durable execution via event-sourced history | **Adopt the invariant**, not the machinery |
| [Camunda](https://docs.camunda.io/docs/components/best-practices/development/dealing-with-problems-and-exceptions/) | BPMN human tasks, incidents, compensation | **Adopt incidents + compensation**, refuse BPMN |
| [Windmill](https://www.windmill.dev/docs/flows/flow_approval) | Approval/suspend steps, per-step retries | **Adopt approval expiry + retry shape** |
| [n8n](https://docs.n8n.io/data/data-pinning/) | Node graph, expressions, pinned data | **Adopt data-pinning and a variable store**, refuse JS expressions |
| [Airflow](https://airflow.apache.org/) | Python DAGs, scheduler, backfill | **Adopt idempotency keys only** |
| [Kestra](https://kestra.io/) | Declarative YAML workflows | Already have it — `WorkflowVersion.steps` is a diffable document |
| [Prefect](https://www.prefect.io/) | Python-first dynamic flows, state tracking | Nothing Ghost needs now |
| [Argo Workflows](https://argoproj.github.io/workflows/) | Container-native K8s DAGs | Nothing — wrong layer |

---

## Temporal — the most important steal

**What it does.** A Temporal workflow's progress is an append-only *event history*.
When a worker dies, another picks up the workflow and **replays the history** to
rebuild in-memory state. The critical rule: side effects (Temporal calls them
*activities*) are **never re-executed during replay** — their recorded results are
handed back instead. Workflow code must be deterministic so the same history yields
the same command sequence; a mismatch raises a non-determinism error rather than
silently doing the wrong thing.

**Why it matters to Ghost.** Before this work, Ghost tracked run progress with a
single `Run.cursor` integer that was only persisted at approval gates and at
completion. Mid-run it was a local variable. A worker crash meant the job was
redelivered and the run restarted from a stale cursor — re-executing completed steps.
And because the approved-step set is rebuilt from the database on resume, an
already-approved **sensitive** step re-planned as `execute` rather than `gate`. A
payment could fire twice with no human in the loop, in a product whose entire promise
is "no silent send/pay/delete."

**What Ghost adopts.**

- **The invariant.** A step with a recorded success is never re-executed. Run state is
  now a *fold over an append-only journal* (`RunEvent`), not a stored cursor. Double
  execution stopped being a bug to fix and became a state the engine cannot represent.
- **Activity options.** Per-step `timeoutMs` and `retry {maxAttempts, backoffMs,
  factor}`, mirroring Temporal's start-to-close timeout and retry policy.
- **Approval as a signal.** A run halts, the worker is freed, and an external decision
  resumes it — structurally Temporal's signal-driven wait.

**What Ghost refuses.**

- **The deterministic-replay VM.** Temporal re-runs your workflow *function* against
  history. Ghost's workflows are declarative step arrays, not code, so there is no
  function to replay — the fold is enough and is far simpler.
- **Workflow versioning ceremony.** Temporal needs `patched`/`getVersion` guards
  because live workflows outlive code deploys. Ghost pins every run to an immutable
  `WorkflowVersion`, which already solves this.

**The part that does not transfer.** Temporal assumes replay reconstructs state.
**Browser state cannot be reconstructed from a journal** — the DOM, the session, and
the server's view of it all live outside. Ghost's answer is documented under
"Restoration" below.

## Camunda — incidents and compensation

**What it does.** When a job exhausts its retries, Camunda raises an **incident**: the
process instance stops and waits for a human to inspect and resolve it in Operate,
rather than dying. Separately, BPMN **compensation events** let each activity declare
an undo handler; triggering compensation invokes the handlers of completed activities
in reverse — the saga pattern, expressed in the model.

**What Ghost adopts.**

- **Incidents.** A failed step no longer kills the run. It moves to `INCIDENT`, and a
  human can retry, skip, or cancel. This matches how Ghost's customers actually work:
  an ops person watching a run wants to fix the one broken step, not restart a
  40-invoice batch.
- **Compensation as the shape of undo.** Ghost promises recovery, and the journal is
  the natural substrate: walk it backwards, invoking each executed step's compensator.
  *Designed, not yet built* — see Deferred.
- **Skip is gated.** Skipping a step that `classifyStep` marked sensitive requires a
  fresh approval. An incident must never become a way around the gate.

**What Ghost refuses.** BPMN 2.0 itself — the XML, the modeler, the notation. Ghost's
users demonstrate a workflow in a browser; they do not draw swimlanes.

## Windmill — approval ergonomics

**What it does.** Approval/suspend steps pause a flow until an external event resumes
it, **with the worker freed while suspended**, plus configurable timeouts and multiple
approvers. Steps carry their own retry counts and error handlers, and scripts carry
concurrency limits.

**What Ghost adopts.**

- **Approval expiry.** `Approval.expiresAt` — an expired approval cannot be consumed.
  Without this, an approve clicked and forgotten could fire a payment days later, which
  is exactly the kind of thing an auditor asks about.
- **Freeing the worker at the gate.** Ghost already closes Chromium when it halts, so
  the instinct was right; the journal now makes resuming from that state safe.

**Noted for later:** multiple approvers and Slack/email approval routing. Both need a
notification surface Ghost does not have yet.

## n8n — data flow, and one firm refusal

**What it does.** Nodes pass items to each other, with a JavaScript-like **expression**
language for referencing upstream output. **Data pinning** freezes a node's output so
you can iterate on downstream nodes without re-hitting live services; production
executions ignore pinned data.

**What Ghost adopts.**

- **A variable store.** Ghost's `extract` step read a value and threw it away. Extracted
  values now persist to the journal and later steps can reference them.
- **Pinned data**, eventually — it is the right answer for the authoring loop, and
  Ghost's implementation-services model means someone is *configuring* workflows as a
  paid activity. Deferred only because it needs the step editor, which does not exist.

**What Ghost firmly refuses: JavaScript expressions.**

Ghost's first engineering rule is *AI may propose; deterministic code executes only
approved plans*. An approved plan containing arbitrary JavaScript is not a plan — it is
a program that can compute a different action at runtime than the one the human
reviewed. Ghost's resolver handles `{{ steps.<id>.<name> }}` and `{{ vars.<name> }}`
and nothing else. No `eval`, no arithmetic, no method calls. An unresolvable reference
is a hard error, never a silent empty string.

Two rules travel with the feature, and both matter more than the feature:

1. **Interpolation happens before classification and before the approval preview.** The
   human approves the *resolved* action. Approving "Pay `{{ vars.amount }}`" and letting
   the engine resolve it afterwards would make the gate theater.
2. **A resolved value re-triggers classification.** A value flowing into a
   sensitive-looking field gates, regardless of where it came from.

## Airflow — one idea, narrowly

Airflow's scheduler, backfill, and operator ecosystem are all aimed at batch data
pipelines. Ghost's workflows are linear, human-triggered, and browser-shaped; almost
none of it applies.

The one steal: **deterministic job keys**. Ghost now enqueues with an explicit
`jobId` derived from the run and its position, so a double-clicked Run button or a
double-submitted approval collapses to a single job instead of racing two workers
against one run.

## Kestra — already have it

Kestra's insight is that a workflow should be a **declarative document** you can diff,
review, and put in version control. Ghost's `WorkflowVersion.steps` is exactly that: an
immutable JSON document validated by a Zod schema, with runs pinned to a version so
editing never mutates what a past run executed. No change needed; worth stating because
it is a property to protect, not lose.

## Prefect, Argo — not applicable

Prefect's dynamic Python flows and Argo's container-per-task Kubernetes model both
solve problems Ghost does not have. Argo in particular operates a layer below Ghost —
it schedules containers; Ghost operates applications.

---

## The synthesis: tamper-evident durable execution

Ghost already kept an append-only, hash-chained audit log. Temporal keeps an
append-only event history. **These are the same data structure serving two purposes**,
and unifying them gives Ghost something none of the surveyed platforms has: the record
that *proves* what happened is the same record the engine *replays* to resume. An
auditor and the scheduler read one artifact.

The chain is two-level:

- **`RunEvent`** — per-run journal, hash-chained, `@@unique([runId, seq])`. Per-run
  rather than per-org because a single org-wide chain serializes every step of every
  concurrent run onto one tail; the previous implementation read the last hash and
  inserted without a transaction, so two concurrent runs could fork the chain.
- **`AuditEvent`** — the org-wide ledger, now recording run-level milestones plus each
  finished run's journal **head hash**. Step detail stays tamper-evident without
  org-wide write contention.

## Restoration: the part no one else solves for us

Temporal's replay works because state is in memory. Ghost drives a *browser*, and when
it halts at an approval gate it closes Chromium — so resume would otherwise start on
`about:blank`. The previous fix replayed every prior step, clicks included, and its own
comment conceded this only held for "correctly authored workflows." With two gates in
one workflow, resuming at the second re-clicked the first one's approved button.

Ghost's replacement never re-runs an effect. Every step type is classified for **replay
safety** (`packages/core/src/classifier/replay.ts`), a sibling of the sensitivity
classifier with the same fail-closed discipline:

- `restorable` — `navigate`, `waitFor`, `select`, `extract`, `verify`, and non-sensitive
  `fill`
- `effectful` — `click`, `apiCall`, `sendEmail`, `approval`, and any sensitive `fill`
- unknown step type — `effectful`, by default

Restoration is then: reload the captured `storageState` (cookies + localStorage), return
to the recorded URL, and re-apply **only** `restorable` prefix steps. If the page cannot
be provably restored, the run raises an incident and asks a human — **fail loud, never
silently replay**. Persistent per-org browser contexts (Phase 2, with recording) are the
real fix; this is the honest interim.

> `storageState` contains session cookies, which are bearer credentials. It is stored
> under a separate, encrypted namespace — never the artifact prefix the web app serves
> to browsers.

## Deferred, with reasons

| Idea | Source | Why not yet |
|---|---|---|
| Pinned data + partial re-execution | n8n | Needs the typed step editor, which does not exist yet |
| Compensation / undo handlers | Camunda | The journal makes it tractable; design it now that the journal exists |
| Triggers and schedules | Airflow, BullMQ | No trigger surface exists at all. Scheduling *unattended* runs of an approval-gated workflow needs a product answer for who gets notified |
| Multiple approvers, Slack/email approval | Windmill | Needs a notification surface |
| Four-eyes approval | Camunda | Orgs are auto-created single-member on first sign-in, and `Role` is stored but never enforced. Needs RBAC first |

## Sources

- [Temporal — Workflow Execution overview](https://docs.temporal.io/workflow-execution)
- [Temporal — Event History walkthrough (TypeScript)](https://docs.temporal.io/encyclopedia/event-history/event-history-typescript)
- [Temporal — Develop code that durably executes](https://learn.temporal.io/tutorials/typescript/background-check/durable-execution/)
- [Camunda — Dealing with problems and exceptions](https://docs.camunda.io/docs/components/best-practices/development/dealing-with-problems-and-exceptions/)
- [Camunda — Incidents](https://docs.camunda.org/manual/7.5/user-guide/process-engine/incidents/)
- [Camunda — How a bank uses compensation events](https://camunda.com/blog/2025/06/how-a-bank-uses-compensation-events-camunda-8/)
- [Windmill — Suspend & Approval / Prompts](https://www.windmill.dev/docs/flows/flow_approval)
- [Windmill — Error handling in flows](https://www.windmill.dev/docs/flows/error_handling)
- [n8n — Data mocking and pinning](https://docs.n8n.io/data/data-pinning/)
- [n8n — Manual, partial, and production executions](https://docs.n8n.io/workflows/executions/manual-partial-and-production-executions/)
