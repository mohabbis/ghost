# Ghost Cloud — handoff for Cursor

You're picking up **Ghost Cloud**, the cloud SaaS rebuild of Ghost: the
**governed execution / trust runtime AI agents plug into**.

```text
Agent (propose) → Ghost (approve · execute · verify · audit)
```

Ghost learns a business workflow once, then executes it across existing software
with **human approval on sensitive actions, verification, and a full audit log**.
Agents may list/preview/start runs via `/api/agent` + `apps/mcp`; they **must
not** approve. All new work lives under `cloud/` (pnpm + Turborepo). The legacy
Rust/Tauri desktop app at the repo root is out of scope unless explicitly
requested.

Read `cloud/README.md`, `cloud/docs/PHASE_1_PLAN.md`, and
`cloud/docs/AGENT_PLUGIN.md` first — this file is the "where we are / what to do
next" layer on top of them.

## Where we are

Phase 0 + Phase 1.1 + Phase 1.2 are **merged on `master`**, and workflows are
now **authorable**: `POST /api/workflows`, `POST /api/workflows/[id]/versions`
and a typed step editor at `workflows/new` / `workflows/[id]`. Editing publishes
a new version rather than mutating one — a `Run` pins its `workflowVersionId`,
so a run in flight keeps the definition it started with.

The loop has been executed end-to-end against a workflow authored **through the
UI**, not only the seeded demo:

```text
author steps → run → AWAITING_APPROVAL on Submit → Approve
  → session restore → submit → verify → SUCCEEDED
```

with exactly one `step.succeeded` for the submit step across the halt. Reject
fails the run without submitting.

Two rules the editor enforces, both worth preserving:

- every step shows its `classifyStep` verdict live, and nothing turns a gate off
- only step types the worker implements are offered. `apiCall`/`sendEmail` parse
  and gate but `applyStep` returns `{}` for them, so authoring one would skip
  the step and report success. Guarded by `EDITABLE_STEP_TYPES`, a server-side
  refusal, and `apps/worker/src/browser/editable-steps.test.ts`

Ghost is licensed **AGPL-3.0-or-later** as of PR #380. Section 13 obliges a
modified network deployment to offer its users source, so the app links it from
the sidebar and sign-in page via `NEXT_PUBLIC_SOURCE_URL` — set that to your own
published source if you run a fork.

### Resume after approval

Ephemeral Chromium is closed when the worker halts at a gate, so on resume the
job would otherwise start on `about:blank` and time out looking for "Submit
order". Restoration is URL-scoped and re-applies only steps classified
`restorative` in `@ghost/core/classifier/replay`; when page state cannot be
provably rebuilt the run raises an incident rather than replaying an action.
Covered by:

- `apps/worker/src/browser/driver.test.ts` (hermetic restore)
- `apps/worker/src/jobs/runWorkflow.test.ts` (Postgres gate → approve → succeed;
  reject → fail) — runs when `DATABASE_URL` is set, otherwise skipped

(An earlier `restoreBrowserPrefix` looped over `applyStep` and would happily
re-click an already-approved Submit. It is gone; see "Phase 1.3" below.)

## Local loop

```bash
cd cloud
cp .env.example .env
#  In .env set at minimum:
#    AUTH_SECRET   -> `openssl rand -base64 32`
#    GHOST_ARTIFACT_DIR -> ABSOLUTE path both web+worker share
#    APP_URL       -> http://localhost:3000
pnpm install
docker compose up -d
pnpm db:migrate
pnpm --filter @ghost/worker exec playwright install chromium
pnpm dev   # or separate web / worker filters
```

Then: sign in (any email) → Workflows → Create demo workflow → Run → Approve.

## Architecture

```
cloud/
  packages/core/         Prisma, Zod steps, classifyStep, audit chain, agent tool catalog
  apps/web/              Next.js 15 + Tailwind v4 + Auth.js v5 + /api/agent/*
  apps/worker/           BullMQ + Playwright execution engine
  apps/mcp/              Stdio MCP bridge → /api/agent/invoke (no approve tools)
```

**Trust model (do not regress):** AI may propose; deterministic code executes
approved plans. The approval gate is enforced in
`apps/worker/src/runtime/state-machine.ts` via `@ghost/core`'s `classifyStep` —
no AI in that decision. Every run/step/gate/finish appends to a per-org
hash-chained `AuditEvent`.

**Execution flow:**

- `POST /api/runs {workflowId}` → `Run` (QUEUED) + enqueue `runWorkflow`
- Worker walks steps from `run.cursor` via `planNextAction`. Sensitive step
  with no APPROVED approval → create `Approval`, `AWAITING_APPROVAL`, stop
  (browser closed).
- `POST /api/runs/[id]/approvals/[stepIndex]` approve → re-enqueue; worker
  restores prefix, executes the gated step, continues. Reject → `FAILED`.
- UI: `run-timeline.tsx` polls every 1.5s.

## Agent plugin surface (shipped)

- Tool catalog: `@ghost/core/agent` (forbid list includes approve/reject)
- HTTP: `GET /api/agent`, `POST /api/agent/invoke`, plus REST mirrors under
  `/api/agent/workflows|runs|approvals` (POST approvals → 403)
- Auth: session **or** a hashed, revocable bearer credential created in Ghost Settings
- MCP: `pnpm --filter @ghost/mcp exec tsx src/index.ts`
- Doc: `cloud/docs/AGENT_PLUGIN.md`

## Phase 1.3 — durable execution (done)

Run position is now a fold over an append-only, hash-chained `RunEvent` journal
rather than a stored cursor. A step with a recorded terminal success is
structurally unreachable, which closes a live double-submit hole: `Run.cursor`
used to be a local variable persisted only at gates, so a crash meant the run
restarted from a stale cursor and — because approvals are re-read from the
database — an already-approved sensitive step re-planned as `execute` instead of
`gate`.

Landed with it: run leases, idempotent job ids, per-step timeout/retry (clamped
to one attempt for anything the classifier gates), Camunda-style incidents with
retry/skip, a cancel route and button, approval decisions written into the audit
chain, `GET /api/audit/verify` plus an `/audit` page, a variable store with a
deliberately non-Turing-complete `{{ }}` resolver, and AES-256-GCM sealing of
captured browser session state under a worker-only key.

`restoreBrowserPrefix` is gone. Restoration is URL-scoped and re-applies only
steps classified `restorative` in `@ghost/core/classifier/replay`; when page
state cannot be provably rebuilt the run raises an incident rather than
replaying an action. Rationale and prior art: `cloud/docs/PRIOR_ART.md`.

### Verifying the queue path by hand

The vitest suite calls `runWorkflowJob` directly, so it does not exercise
BullMQ delivery, job-id idempotency, or a real worker process. With Postgres,
Redis, the web app and the worker running:

```bash
pnpm --filter @ghost/worker exec tsx src/e2e-drive.ts
```

It seeds a run against `/fixtures/order`, enqueues it, waits for the gate,
approves, and prints the journal. A healthy run shows `session.captured` at the
gate, `session.restored` on resume, and exactly one `step.succeeded` for the
submit step.

### Known gaps in durable execution

Three findings from the review of PR #373 were deferred rather than patched
under a merge deadline. All three are now closed, the last (the org chain's
missing expected-head column) partially — see below for what's still open.

**Closed — session blobs are bound to their run.** `seal`/`open` take associated
data and session state is sealed under `ghost:session:v1:<runId>:<artifactKey>`,
so a blob cannot be replayed into another run or another gate. `openSession`
also verifies the blob against the SHA-256 the journal recorded at capture
before decrypting it. Previously encryption proved only that a blob was
authentic under the worker key, which every run shares.

**Closed — cancellation is one transition.** The route's status change, approval
invalidation and journal append commit together, and it anchors the journal head
the way a worker-side terminal transition does. Both sides skip the append if
the other got there first, so a running job and the route can no longer produce
two `run.canceled` events straddling an anchor.

**Closed — a truncated journal tail is detected before resume.** Every
`appendRunEvent` now updates `Run.journalHead` in the same transaction. Forward
and compensation workers compare the surviving tail with that expected head in
addition to verifying internal links. A mismatch quarantines the run without
appending to the compromised chain (which would otherwise bless the forged
tail); the finding is written to the independent organization audit chain.

This protects against deletion from `RunEvent`, including deletion of the whole
journal. Because the expected head is in the same database, an attacker able to
rewrite both the journal and `Run.journalHead` remains outside this threat model.

**Closed (same-database half) — the org chain now has an expected head too.**
`Organization.auditChainHead` mirrors `Run.journalHead`: `appendAuditEvent`
updates it in the same transaction as every event, and refuses to append
(`OrgAuditChainIntegrityError`) if the surviving tail doesn't match it first.
`/api/audit/verify` reports `org.expectedHeadMatches` alongside `org.intact`,
the same shape as the existing `run.expectedHeadMatches`. This closes the case
where deleting a valid *suffix* of `AuditEvent` used to verify as an intact,
merely shorter, chain. Existing orgs were backfilled from their current tail on
migration. Covered by `packages/core/src/auditLog.test.ts`.

**Still open — the org chain is not a stronger seal than that, and should not
be described as one.** `Organization.auditChainHead` is a same-database column,
same as `Run.journalHead` — an attacker able to rewrite both `AuditEvent` and
that column together remains outside this threat model, exactly as rewriting
both `RunEvent` and `Run.journalHead` does. Run-boundary anchors do mean
tampering with a run journal contradicts a value already recorded in the org
chain, but only while the org chain's own tail is intact by the same standard.

Nothing in the system is anchored outside the database. Until something is —
periodic export of the org head to storage the app cannot write, a countersigned
receipt, anything — the honest scope is: this detects corruption and partial
edits (including, now, a deleted suffix), not an attacker with write access to
the whole database. Say that plainly rather than implying a seal that does not
exist.

### Known gaps in compensation (undo)

Three findings from the review of PR #374 were deliberately left open. One is
now closed; the other two still need a decision rather than a patch, and
neither is safe to forget.

**A reversal runs in an unauthenticated browser.** `compensateRunJob` launches a
fresh context with no storage state, and forward completion deliberately purges
the run's captured session. So a compensation against any system behind a login
— which is most of them — lands on a sign-in page and fails. The reversal is
reported honestly (the run stops as an incident, nothing is claimed as undone),
but the feature only really works against unauthenticated targets today.

Closing it means keeping a run's credentials alive past the run's end, which is
the opposite of the current rule that browser credentials must not outlive the
run that captured them. Options: retain the session blob for a bounded undo
window and say so in the product; or re-authenticate at reversal time from the
connector grant once connectors exist. The second is the better shape and is
blocked on connectors, so the first is the honest interim — but it is a
credential-retention decision, not a code change, and it should be made
deliberately rather than by whoever touches this file next.

**Closed — cancellation can no longer anchor ahead of an in-flight step.**
Cancelling a run whose current step was mid-execution used to let the route
read and seal the journal head before that step committed its
`step.succeeded`, leaving the final head not matching the sealed anchor —
which verification reported as tampering on a run that was never tampered
with. The cancel route now checks the run's lease in the same transaction as
the status flip: with no active lease, it finalizes (appends `run.canceled`
and anchors) exactly as before, since nothing is executing. With an active
lease, it flips status only and defers to the worker's own loop-top CANCELED
check, which now finalizes after any in-flight step has actually committed —
safe because it's the same process doing both writes in order. The route's
cleanup re-enqueue is delayed past the lease TTL in that case, and the
worker's terminal guard gained the matching lease check so it can safely
finalize an abandoned (crashed-mid-step) cancellation instead of only a
live one. Covered by
`apps/web/src/app/api/runs/[id]/cancel/cancel.test.ts`.

**The AAD change has no story for blobs sealed before it.** Session blobs are
now sealed with associated data binding them to their run and gate. A blob
written by the previous code has none, so opening it fails authentication, the
catch falls back to an empty context, and an authenticated run waiting at a gate
across the deploy cannot resume. There is no deployed environment holding such
blobs today, which is the only reason this is not a migration task; if that
changes before this is addressed, version the blob format and accept both.

## Read this before trusting a green PR

**`cloud/` has CI** (`.github/workflows/cloud.yml`, path-scoped to `cloud/**`),
activated in PR #393. It runs against real Postgres + Redis services, not a
placeholder `DATABASE_URL`, so the ~90 database-gated tests actually execute
rather than silently skipping. `rust.yml` is scoped away from `cloud/`-only
changes as part of the same PR, so a cloud PR's green checks now mean the
changed code compiled, typechecked, and passed tests — not ~22 unrelated Rust
checks that never touched it.

Before #393, `cloud/ci/cloud.yml` existed but wasn't picked up by GitHub
(workflow files outside `.github/workflows/` are never run), so this section
used to say "no CI" and tell you to validate locally. That gap is closed; the
docs below are historical context for why local validation still matters.

Still validate locally when iterating **with `DATABASE_URL` set and Postgres +
Redis running** — CI catches it before merge either way, but a local loop is
faster than waiting on a run:

```bash
cd cloud && pnpm typecheck && pnpm test && pnpm build   # expect 239 tests
```

Without a database roughly 90 of those skip themselves and you learn nothing
about the engine. If the worker suite reports 45 rather than 90, the database is
not being reached.

## Remaining Phase 1 work (priority)

Done: **credential hardening** — optional expiry (`expiresInDays` on
`POST /api/settings/agent-credentials`) and an organization-admin inventory +
revocation view (`GET /api/settings/agent-credentials/org`, and `DELETE
/api/settings/agent-credentials/[id]` now permits an OWNER/ADMIN to revoke a
colleague's credential, not only their own). `packages/core/src/roles.ts`
(`isOrgAdmin`) is the first thing to actually read `Membership.role` — every
existing org still has exactly one OWNER member, so this changes nothing
observable until an org has more than one member.

1. **SSE/WebSocket** for the timeline (nice-to-have; polling works).
2. **S3 serve path** — disk store works in dev; wire presigned URLs when S3 is on.
   Note the artifact route is now a positive allow-list (`step-`/`restore-` PNGs
   only); keep it that way, because the same prefix holds encrypted session
   blobs that must never be served.
3. **Compensation / undo handlers** — the journal makes the saga pattern
   tractable: walk it backwards invoking each executed step's compensator.
4. **Four-eyes approval** — `isOrgAdmin` above is a role check, not the RBAC
   `requireSeparateApprover` would need (which is about who may *approve*, not
   who may *administer*); still needs its own design, and orgs are still
   auto-created single-member on first sign-in with no invite flow, so it has
   no real subject to test against yet.

## Phase 2

Recording → editable workflow. Open decision: server-side remote cloud browser
(recommended) vs Chrome extension. Engine boundary is clean either way.

## Repo conventions

- Validate from `cloud/`: `pnpm typecheck && pnpm test && pnpm build`.
- Pass `DATABASE_URL` (and a migrated DB) for the worker integration tests.
- API routes / jobs stay org-scoped via `auth()`; keep the trust pipeline intact.
- CI: `.github/workflows/cloud.yml`, path-scoped to `cloud/**`, runs on every
  push/PR against real Postgres + Redis services — see the section above.
- Canonical product docs: `AGENTS.md`, `CLAUDE.md`, `cloud/README.md`. Root
  `README.md` should describe the cloud product; the desktop app is legacy.
