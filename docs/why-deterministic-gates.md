# Why deterministic approval gates beat autonomous agents

An agent that can do the work can also do the damage. The industry's answer has
mostly been to make the agent more careful — better prompts, a self-critique
pass, a confirmation step the model decides to take. Ghost's answer is that the
decision to pause is not the model's to make.

This document explains that position through the places it actually shows up in
the code, because the argument is only worth as much as its edge cases.

---

## The claim

> A system that executes real business actions should be judged by what it does
> on its worst day, not its median one.

On a median day an autonomous agent and Ghost look identical: the work gets
done. The two diverge on the day the page layout changed, the network hiccuped
mid-click, the workflow author wrote something careless, or the model was simply
confident and wrong. Those are the days that decide whether a business can put
an automation near its money.

So the design rule is narrow and absolute:

> **AI may propose. Deterministic code decides what requires a human, and
> deterministic code executes only what a human approved.**

The gate is a pure function over the step definition — a word list, essentially
(`packages/core/src/classifier/sensitive.ts`). It is unglamorous on purpose. A
classifier you can read in one sitting is a classifier you can audit, test
exhaustively, and reason about when it is wrong. A model that gates "usually"
has no such property, and "usually" is not a security control.

The rest of this document is the interesting part: four places where "just add
an approval step" is not enough, and what had to be true instead.

---

## 1. The retry that was a second payment

Every serious execution engine retries. A click times out, the network was slow,
you try again — this is table stakes, and every workflow engine from Temporal to
Airflow gives you a retry policy with backoff.

Now apply it to a step labelled **Submit order**.

A timeout is not a failure. It is the *absence of information*. The request may
never have left; it may have arrived, been processed, and the response lost on
the way back. Retrying resolves the ambiguity in exactly one direction: it
assumes nothing happened. When the assumption is wrong, the customer is charged
twice, and the audit log faithfully records two successful submissions as if
both were intended.

Ghost refuses to let a workflow author configure this:

```ts
// apps/worker/src/runtime/policy.ts
export function effectiveRetry(step: WorkflowStep): RetryPolicy {
  if (isAtMostOnce(step)) return NO_RETRY;
  return step.retry ?? NO_RETRY;
}
```

Three things are load-bearing here.

**It is a clamp, not a default.** A stored workflow with `retry.maxAttempts: 5`
on a payment step does not get 5 attempts. The engine reads the definition and
declines. Defaults are advice; clamps are guarantees, and the difference matters
when the definition can be edited by someone who has not thought about
double-submission.

**It is enforced at execution, not at authoring.** Validation at write time
protects you from the workflows you know about. Enforcement at execution
protects you from every path that ever writes a workflow row — the editor, a
migration, a future recorder, an API client, a bug.

**The clamp reuses the gate's own classifier.** `isAtMostOnce` is the same
sensitivity judgment that decides whether to pause for a human. One definition of
"this action is irreversible", consumed by both the approval gate and the retry
policy, cannot drift into the state where a step is dangerous enough to need
approval but safe enough to retry automatically.

---

## 2. The step whose outcome nobody knows

The clamp above stops a *deliberate* retry. There is a harder case: the worker
died mid-step. The process is gone, and with it any knowledge of whether the
click landed.

Most engines resolve this with a policy — at-least-once (re-run it) or
at-most-once (skip it). Both are wrong here, and for the same reason: they are
guesses presented as decisions. Re-running may double-charge. Skipping may leave
an order unsubmitted while the run reports success.

Ghost's state machine has a third answer:

```ts
// apps/worker/src/runtime/state-machine.ts
| { kind: "indeterminate"; index: number; step: WorkflowStep; reason: string }
```

> An at-most-once step was interrupted mid-flight. Never auto-resume: we cannot
> tell whether the effect landed, and neither re-running nor skipping is the
> engine's decision to make.

The run stops and raises an incident for a human, who can look at the target
system and say what actually happened. This is worse automation and better
engineering. The alternative — picking a branch and being right most of the
time — produces a system whose failures are silent, and silent failures are the
ones that destroy trust in an automation platform.

Note what this costs. It means Ghost cannot promise unattended execution through
arbitrary failures, and a competitor's demo will look smoother. That is the
trade being made deliberately, not a limitation waiting to be fixed.

---

## 3. Approving something other than what runs

Here is the subtlest one, and the one most likely to be got wrong by a system
that bolted approvals on afterwards.

Workflow steps carry template references — `Pay {{ steps.invoice.total }}`. A
human at the gate sees a preview of the step and clicks Approve.

If the engine resolves those templates *after* the approval, the human approved
a string with a hole in it. They authorized "pay the invoice total"; what
executed was "pay $48,000", a number they never saw and could not have objected
to. The gate would still be there. Every log line would look correct. And the
control would be worthless, because consent to an unspecified amount is not
consent.

So resolution happens before classification, and the state machine says why:

```ts
// apps/worker/src/runtime/state-machine.ts
/**
 * Resolves a step's `{{ }}` references against values captured earlier in the
 * run. Applied to the candidate step *before* it is classified, so the gate
 * decision and the approval preview describe the action that will actually
 * run — approving `Pay {{ steps.total.amount }}` and resolving it afterwards
 * would make the gate theater.
 */
export type StepResolver = (step: WorkflowStep, index: number) => WorkflowStep;
```

The ordering has a second effect that is easy to miss: because resolution feeds
the classifier, a step can become sensitive *as a result of its own data*. A
generic "click the button named `{{ action }}`" step where `action` resolves to
"Delete account" gates on the resolved text, not the template. A classifier
running on unresolved templates would have seen nothing worth stopping for.

**The generalizable rule:** an approval gate is only as meaningful as the
fidelity between what the human was shown and what the machine then did. Any
transformation applied after the human's decision is an unreviewed change.

---

## 4. Deriving position instead of remembering it

Durable execution is where trust systems quietly break. Ghost's run engine does
not store a cursor. It folds the run journal:

```ts
// Linear scan rather than a cursor, deliberately. Workflows are tens of
// steps, and the O(n) walk is what removes the mutable position that caused
// completed steps to re-execute. Do not "optimize" this back into a cursor.
```

The history is worth stating plainly, because it is the strongest argument in
this document. An earlier version *did* keep a cursor, persisted only at gates.
A redelivered job would resume from a stale position and re-run completed steps.
And because approvals are re-read from the database, an already-approved payment
re-planned as `execute` rather than `gate` — the second charge went through
without stopping, precisely *because* it had been legitimately approved once.

That is the shape of failure this whole architecture exists to prevent, and it
was not caused by a missing approval gate. The gate was there. It was caused by
derived state and stored state disagreeing.

Making position a pure function of the journal means "this step already
succeeded" is not a fact the engine has to remember correctly — it is a state
the planner cannot return. The class of bug is removed rather than patched.

---

## 5. Who counts as a human

A gate that any authenticated party can satisfy is an audit finding, not a
control. Two separate restrictions:

**Agents cannot approve, structurally.** Not "are instructed not to". The
`POST` handler on `/api/agent/approvals` exists only to return a hardcoded
`403` — it takes no arguments, reads no session, and never reaches the approval
logic at all:

```ts
export async function POST() {
  return NextResponse.json(
    { error: "Agents cannot approve or reject. Open the run in the Ghost UI; a human must decide." },
    { status: 403 },
  );
}
```

It is a handler purely so the refusal is explainable rather than a bare `405`.
Behind it, `FORBIDDEN_AGENT_TOOLS` keeps the approve/reject verbs out of the
tool catalogue an agent can even enumerate. An agent may propose a workflow,
start a run, and read pending approvals; a prompt injection cannot talk it into
approving its own payment, because there is no code path to talk it into.

**Optionally, the human who started a run cannot approve it**
(`Workflow.requireSeparateApprover`). Note the default is `false`, and the reason
is worth repeating because it is the honest kind of limitation: `ensureUserOrg`
creates a single-member organization on first sign-in and there is no invite
flow, so almost every org contains exactly one person. On by default would
deadlock every run at its first gate. The control is built; the multi-user
substrate it needs is not yet.

Rejection is deliberately never restricted. Rejecting halts an action rather
than authorizing one, and whoever started a runaway run must always be able to
stop it. Restricting the stop button to protect a separation-of-duties property
would trade a real safety property for a paperwork one.

---

## What the audit chain does not prove

Every run and approval appends to a SHA-256 hash chain
(`packages/core/src/audit.ts`). It is genuinely useful and it is routinely
oversold, including by projects that ship it. Here is the boundary, pinned by
tests in `packages/core/src/audit.test.ts` that assert `intact: true` on
tampered input.

**It detects:** mutation of a historical payload, a broken `prevHash` link, a
deleted middle event, reordered events, and a forged event spliced in. Each
breaks a link that verification recomputes.

**It does not detect tail truncation.** Nothing in the chain commits to its own
length, so lopping off the last N events leaves every surviving link valid. For
*run journals* this is mitigated out-of-band: `Run.journalHead` stores the
expected tail in the same transaction as every append, so a truncated journal
raises `RunJournalIntegrityError` rather than being blessed with a new head. The
org-wide `AuditEvent` chain has no equivalent column, so truncation there is
detectable only against an external anchor.

**It does not detect a wholesale rewrite.** An attacker who can write every row
can rewrite history and recompute every hash forward. The result verifies as
perfectly intact. This is not a bug in the implementation; it is what a bare
hash chain is. **A hash chain proves internal consistency, not authenticity.**

Closing that gap requires an anchor the attacker cannot recompute — a signed
chain head, an append-only/WORM replica, or periodic publication of the head
hash somewhere outside the database. Ghost does none of these yet. Until it
does, the correct claim is "tamper-evident against anyone who cannot write to
the audit table", and the incorrect claim is "tamper-proof".

Stating that plainly is not a weakness in the pitch. A buyer's security reviewer
will find this in ten minutes; the only question is whether they find it in your
documentation or in your marketing's contradiction.

---

## The trade, stated honestly

Deterministic gating costs real things:

- **Coverage.** A word-list classifier misses sensitive actions phrased in ways
  it does not know, and gates harmless ones that happen to match. A model would
  generalize better. The bet is that a wrong-but-inspectable rule beats a
  usually-right-but-opaque one when the failure is a payment.
- **Autonomy.** Indeterminate steps stop. Runs wait for humans. Ghost cannot
  promise a workflow that never needs anyone.
- **Authoring effort.** Reversibility is not inferred; someone writes the
  compensation. Steps without one are reported as unreversible rather than
  quietly assumed fine — which is exactly what the run detail screen says after
  the demo workflow completes.

What it buys is the ability to answer "what did the automation do to my systems
last Tuesday, who authorized it, and can you prove that record wasn't edited"
with something better than a model's recollection.

That answer is the product. The browser automation is just how it reaches the
work.

---

## Further reading

- `cloud/packages/core/src/classifier/sensitive.ts` — the gate, in full
- `apps/worker/src/runtime/state-machine.ts` — planning, gating, indeterminacy
- `apps/worker/src/runtime/policy.ts` — the retry clamp
- `cloud/packages/core/src/audit.ts` — the chain and its verifier
- [`trust-pipeline.md`](trust-pipeline.md) — the pipeline these rules implement
- [`threat-model.md`](threat-model.md) — cloud threats and controls
