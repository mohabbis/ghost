# Trust pipeline

Every meaningful Ghost Cloud operation:

```text
Intent → Plan → Policy check → User approval → Execution → Audit log → Undo/Recover
```

In the cloud MVP this maps to:

| Stage | Where |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Intent | User triggers a run / later: a recording |
| Plan | Immutable `WorkflowVersion` steps |
| Policy | `classifyStep` in `@ghost/core` (deterministic) |
| Approval | `Approval` row + UI; run `AWAITING_APPROVAL` |
| Execution | Worker Playwright / later connectors, under a per-workflow concurrency cap |
| Verify | Per-step assertions + screenshots |
| Audit     | Per-run hash-chained `RunEvent`, with its expected head stored atomically on `Run`, anchored into the org-wide `AuditEvent` chain at run boundaries |
| Recover | Cancel between steps; incident retry/skip; undo (compensation) |

## Recover

Undo is not a future feature — it ships as **compensation**, Ghost's take on the
BPMN saga. A step may carry a `compensate` block: a description plus a short
list of browser actions. Reversal walks the run journal backwards, so it undoes
what the run is _recorded_ to have done rather than what its definition says it
would do.

The reversal is not a privileged path. It goes through the same pipeline:

| Stage | In a reversal |
| --------- | ------------------------------------------------------------------------------------------------ |
| Plan | `planCompensation` over the verified run journal, newest first |
| Policy    | `classifyCompensation` — _stricter_ than forward: every click and every fill gates               |
| Approval | `COMPENSATION`-phase `Approval`, so it cannot be confused with the forward gate on the same step |
| Execution | The compensation worker, one run at a time, interruptible between reversals |
| Verify | The compensation's own assertion, plus a screenshot under its own key |
| Audit | Same hash chain, attributed to whoever requested or approved the undo |

Two things it refuses to pretend. A completed side effect with no defined
reversal is recorded as `step.irreversible` and shown, rather than omitted — a
run reported as fully undone while the confirmation email sits in a customer's
inbox would be the exact lie this pipeline exists to prevent. And a reversal
that fails stops the run rather than pressing on through a half-undone state.

Compensation has known gaps, listed in `cloud/docs/CURSOR_HANDOFF.md` under
"Known gaps in compensation". The one that limits what it is useful for today:
reversals run in a fresh, unauthenticated browser context.

## Exception routing

A run that stops does not just sit in `INCIDENT` waiting to be found by run id.
The stop is **classified when it is raised** by a third deterministic classifier,
`classifyException` — sibling to `classifyStep` (must a human approve this?) and
`replaySafety` (is it safe to re-apply silently?). It answers a different
question: *this stopped — whose desk, and is retrying safe?*

Nine kinds each route to one **owner**, and the owner is the point: a changed
selector (`TARGET_MISSING`) belongs to whoever maintains the workflow, an expired
credential (`AUTH`) or a lapsed gate (`APPROVAL_EXPIRED`) to an administrator, a
rejected value (`DATA`) or a wrong outcome (`VERIFICATION`) to the operator
running the work. "A run stopped" is not actionable; "the portal changed and step
4 needs re-recording" is.

Like its siblings it is a rule table, not a model call, and it fails closed: an
unrecognized reason stays `UNKNOWN` rather than being forced into the nearest
bucket, because a confidently wrong label defeats the purpose of routing.

Two properties carry the trust weight:

- **Classification never changes how a run *stops*.** The worker's transition
  into `INCIDENT` is identical whatever the verdict — the kind decides what a
  human is shown and whose desk the work lands on, not whether the run halts.
  It does, deliberately, shape *recovery*: the disposition feeds
  `duplicateRiskFor`, which is what makes the route below refuse an
  unacknowledged retry. Naming that plainly matters, because a reader who
  believed classification were inert would not think to check it when a retry
  is refused.
- **`OUTCOME_UNKNOWN` retries require acknowledgement.** A step that started and
  never reported back may already have taken effect. The engine still lets a
  human retry it — only a person can check the target system — but the route
  refuses an unacknowledged retry and records the acknowledgement in the run
  journal and the audit log. Duplicate risk additionally requires the step to
  reach *outside the browser*: an indeterminate `verify` is a read, and a prompt
  that fires on reads is one operators learn to click through.

Resolution (`retry` / `skip` / `assign`) stays on the run's own incident route
under its own authorization; the queue at `/exceptions` and `GET /api/exceptions`
is read-only. Assignment is open to any member — deciding who looks at a problem
is not authorizing the action that caused it — and is tenant-scoped to members of
the org that owns the run. Routing fields are cleared whenever a run leaves
`INCIDENT`, and a failed *reversal* is re-routed from scratch rather than
inheriting the forward incident's kind, owner, or waiting time.

## Execution limits

A workflow may cap how many of its runs are in flight at once
(`Workflow.maxActiveRuns`, Airflow's `max_active_runs`). Opt-in, null by default.
Held-back runs wait in `QUEUED` and say why; a slot is taken at admission and
given back explicitly rather than inferred from status, because a cancel drain
and an approval resume both move a run's status while it is still occupying the
customer's system. Full contract in `cloud/docs/CONCURRENCY.md`.

## Rules

1. AI never decides whether a step is sensitive.
2. Approval is a one-way gate the engine cannot skip.
3. A run is not “done” because a click happened — verification must pass.
4. Secrets never land in logs, screenshots, or model prompts.
5. Every mutating API route / job is org-scoped via auth.
6. Undoing an action is itself an action: classified, gated, verified, audited.
7. A run may resume only when both its internal hash links and its independently
   stored expected journal head verify; a missing valid suffix quarantines the
   run rather than making a completed side effect reachable again.

Desktop Organizer had the same _principles_ with different mechanics; see
`legacy/` only when maintaining that code.
