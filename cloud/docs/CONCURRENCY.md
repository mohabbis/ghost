# Per-workflow concurrency (`maxActiveRuns`)

Ghost's take on Airflow's `max_active_runs`. A workflow may cap how many of its
runs are in flight at once; the rest wait rather than being dropped.

This exists because the systems Ghost drives are not data warehouses. A workflow
that fires on every inbound email can otherwise put ten browser sessions into a
customer's ERP simultaneously, and the ERP is exactly the thing that will not
tolerate it.

## The setting

`Workflow.maxActiveRuns` — an integer between 1 and 100, or `null` for no cap.

- **Opt-in.** `null` by default, and every workflow that existed before this
  feature stays `null`. Guessing a number for someone else's system is worse
  than leaving it off.
- **Per workflow, not per version.** Publishing v4 must not double the load the
  workflow is allowed to place on the system it operates.
- **Zero is rejected.** A cap of 0 would silently stop every run — that is a
  disable switch wearing a concurrency setting's clothes, and it belongs behind
  its own explicit control.

### `PATCH /api/workflows/[id]`

```jsonc
// request
{ "maxActiveRuns": 3 }   // or null to clear the cap

// 200
{ "id": "clx…", "maxActiveRuns": 3 }
```

`400` on a non-integer, on `< 1`, or on `> 100`. `404` when the workflow is not
in the caller's organization — the read is org-scoped before the write, so a
workflow id alone is never enough to reconfigure another tenant's workflow.

The read, the write and the audit append commit in **one transaction**. A cap
governs how much load Ghost may place on a customer's system, so a change to it
without a matching durable record is a governance gap, and two concurrent PATCHes
must not both record the same stale `from`.

Raising or clearing a cap wakes the runs already waiting on it. New capacity that
nobody is told about is not capacity.

## A slot is taken and given back, not inferred

`Run.slotHeldAt` records ownership directly. Deriving it from `Run.status` is
simpler and wrong, because status moves for reasons that have nothing to do with
whether a run is still touching the customer's system:

- **Cancellation.** The cancel route marks the run `CANCELED` immediately, but
  the worker deliberately lets the current browser action finish. Status-derived
  ownership frees the slot at the start of that drain, and a second run is
  admitted alongside one that is still mutating the system.
- **Approval resume.** An approved run passes through `QUEUED` on its way back to
  `RUNNING`. In that gap it appears to hold nothing, so another run can take the
  slot and the approved mid-flow run ends up throttled behind it.

Both are the exact failure the cap exists to prevent, so ownership is explicit.

A run takes a slot at admission and gives it back on reaching one of
`SLOT_RELEASING_STATUSES` (`SUCCEEDED`, `FAILED`, `CANCELED`, `COMPENSATED`,
`INCIDENT`) — whatever the status does in between.

### Why `INCIDENT` releases and `AWAITING_APPROVAL` does not

`INCIDENT` releases: a run that failed and is waiting on a human is not touching
anything, and one broken run must not wedge its workflow forever. It re-enters
admission when a human retries it, so the cap still governs what executes.

`AWAITING_APPROVAL` holds. An approval is part of a run's normal flow and is
expected to resolve in minutes, and letting a later run overtake one parked at
its gate would interleave two runs against the same system.

**The cost is real and worth stating:** a cap of 1 plus an approval nobody
answers for a day stalls every later run of that workflow. That is visible in the
timeline rather than silent, and it is the safer of the two failure modes. It is
one constant in `packages/core/src/runtime/concurrency.ts` if the trade-off ever
proves wrong in practice.

## Admission

`claimSlot` (`apps/worker/src/runtime/slots.ts`) reads the cap, counts the
holders, and takes the slot **inside one advisory-locked transaction**
(`pg_advisory_xact_lock` on `ghost:concurrency:<workflowId>`). Every part of that
matters:

- Reading the cap outside the lock lets a PATCH lowering an uncapped workflow to
  a finite cap slip in, so a stale `null` admits a run into a full workflow.
- Counting in one statement and claiming in another lets two workers each see
  room and both take the last slot — the cap would hold everywhere except under
  the load it exists for.

Admission runs *after* the run lease, so it only ever races other runs, never
another worker on the same run. A run that already holds a slot is always
admitted: otherwise a cap of 1 would deadlock a run against itself on resume.

Reversals go through the same admission (`runningStatus: "COMPENSATING"`). An
undo occupies the customer's system exactly as a forward run does.

## Waiting, and being told about it

A throttled run stays `QUEUED` — throttling is not a failure and must not read
as one — and journals `run.throttled` with the cap, the holders, and a sentence
an operator can act on.

The event is re-appended only when what an operator would read has actually
changed. Writing one per re-check would bury a run's real events under hundreds
of identical hash-chain entries; suppressing on "the last event was also a
throttle" would leave the timeline pointing at a holder that finished hours ago.
The run-detail API reads the **holders live** for the same reason.

"Queued" alone does not tell an operator whether Ghost is busy, broken, or
deliberately holding the run — and that distinction is what a customer is asking
about when they say nothing happened.

## Handing a slot on

`releaseSlot` clears `slotHeldAt` and picks the next runs **under the same lock
as admission**, waking as many as there is capacity for. Waking exactly one per
release is not enough: two holders finishing at once both select the same oldest
waiting run, leaving a slot idle.

Two shapes of waiting exist and they resume through different queues — a forward
run sits in `QUEUED`, a refused reversal sits in `COMPENSATING` holding no slot.

Every throttled run also schedules a slow (5 minute) self re-check as the safety
net for a worker killed between finishing and nudging. That job carries
`throttleRecheck: true` and no-ops unless the run is still `QUEUED`: without the
marker, a run handed a slot in the meantime and since parked at an approval gate
would be re-driven by the stale job, appending another `run.resumed`, another
`gate.opened` and another approval audit entry that no human asked for.

Over-waking is cheap — a second job finds the run leased, or throttled again, and
returns. Under-waking stalls a workflow.

## Known limits

- **No org-wide or global cap.** This is per workflow only; an org running twenty
  capped workflows can still saturate the worker pool. `WORKER_CONCURRENCY` is
  the only bound there.
- **The safety-net re-check is a fixed 5 minutes**, not adaptive.
- **Admission has no concurrency stress test.** Correctness under a real race
  rests on the advisory lock plus the claim being in the same transaction —
  reasoned, not measured.
