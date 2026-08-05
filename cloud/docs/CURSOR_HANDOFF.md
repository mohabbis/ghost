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

**Before starting anything new**, read
`cloud/docs/ARCHITECTURE_DECISIONS.md`: it records the four decisions that were
open (HarnessRouter's boundary, enterprise auth, capture, and the audit), the
order the remaining work goes in, and what is deliberately not being built yet.

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
submit step. Verified passing.

### The crash-recovery driver

```bash
pnpm --filter @ghost/worker exec tsx src/crash-drive.ts
```

Same setup, but it spawns its own worker, waits until the submit step is
genuinely mid-flight (`step.started`, no outcome), `SIGKILL`s the worker's
process group, and starts a fresh one. Verified: the recovering worker does
**not** re-click Submit — it raises `step.outcome_unknown` and ends the run
`INCIDENT` with "step 2 started but never recorded an outcome, and its effect
cannot safely be repeated". Zero duplicate submits.

That INCIDENT is the correct outcome, not a failure of the driver. `step.started`
is written before the effect so a crash leaves the step visibly in flight and
the state machine can refuse to guess whether it landed.

If you change this driver, keep the process-group kill and the post-kill
liveness check. An earlier version spawned through `npx` and signalled the
wrapper; the real worker survived, finished the step, and the driver printed
`PASS` having tested nothing.

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
cd cloud && pnpm typecheck && pnpm test && pnpm build   # expect 424 tests
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

Done: **compensation / undo handlers**. This item was stale — the engine
(`packages/core/src/runtime/compensate.ts`'s reversal planner and
`apps/worker/src/jobs/compensateRun.ts`'s 589-line job: journal verification,
in-flight detection, `COMPENSATION`-phase approval gating, re-entrant
execution, incident handling, audit trail) has been complete and covered by
`compensateRun.test.ts` since PR #374. What was actually still missing was one
level up: a step is only reversible if its workflow definition carries a
`compensate` block, and `workflow-editor.tsx` had no fields to author one — so
in practice every workflow created through the product had zero reversible
steps, engine notwithstanding. Closed by adding undo authoring (description +
ordered actions + optional verify assertion) to the step editor for
click/fill/select, the only editable step types with a side effect to reverse.
Manually verified end-to-end in a browser: authored a click step's undo,
saved, confirmed the persisted `WorkflowVersion.steps` JSON matches
`compensationSchema` exactly, reloaded the editor and confirmed every field
round-trips. No component-testing infra exists in this codebase to cover this
with an automated test (no testing-library/jsdom in `apps/web`); typecheck and
build are the automated coverage this has.

Done: **SSE for the run timeline**, replacing the client's old 1.5s
`setInterval` poll of `GET /api/runs/[id]`. `apps/web` deploys to Vercel
serverless functions (`docs/DEPLOY.md`) with an execution-time limit — 10s
default, nothing in this app raises it — so `GET /api/runs/[id]/stream`
deliberately does **not** hold one connection open for a run's whole
lifetime; it bounds each connection to an 8s segment and lets the browser's
native `EventSource` reconnect on its own for the next one, closing for real
only once the client observes a terminal status. Within a segment it polls
the DB at 400ms and only pushes a frame when the serialized view actually
changed. `buildRunView` (the ~140-line view builder) moved out of the old
route into `apps/web/src/lib/run-view.ts` so the plain-fetch route and the
SSE route share one implementation rather than risking two that drift.
Verified with `stream.test.ts` (3 tests, including a deterministic
read-then-react-to-each-frame drive through QUEUED → RUNNING → SUCCEEDED —
not a `setTimeout` race against the poll loop, which turned out to be
genuinely flaky here for reasons not fully run to ground) and manually
end-to-end in a browser: watched a run's status update live via SSE with no
page reload, confirmed the stream closes on a terminal status rather than
reconnecting forever (5 requests total, network panel stayed flat for 8s
after).

Done: **S3 serve path**. `packages/core/src/storage/artifacts.ts` has a real
`S3ArtifactStore` (put/get/delete/deletePrefix/presigned `signedUrl`) alongside
the disk fallback; `artifactStore()` switches on `S3_BUCKET` +
`S3_ACCESS_KEY_ID` + `S3_SECRET_ACCESS_KEY` being set, and `docs/DEPLOY.md`
documents the silent-fallback failure mode. The artifact route is a positive
allow-list (`step-`/`restore-` PNGs only) — keep it that way; the same prefix
holds encrypted session blobs that must never be served. What's still actually
missing is a retention/cleanup policy: nothing expires or purges old run
artifacts, on disk or in the bucket, so storage grows unbounded over the life
of an org. Not started.

Done: **four-eyes approval**. `isOrgAdmin` above undersold this — a full
membership model shipped alongside it: `Membership`/`Role`
(`OWNER`/`ADMIN`/`MEMBER`) and `Invitation` in the schema, invite/accept routes
under `app/api/invitations` and `app/api/settings/members`
(email-match-enforced acceptance, not just token possession — see
`checkInvitationRedeemable`), and `Organization.requireSeparateApprover`
enforcing `requester_user_id !== approver_user_id` server-side when a workflow
opts in. Covered end-to-end by
`apps/web/src/app/api/runs/separation-of-duties.test.ts` (5 tests) and
`apps/web/src/app/api/settings/members/members.test.ts` (9 tests) against a
real Postgres. Nothing on this item is a gap anymore.

## Phase 2

Recording → editable workflow, split into Capture and Convert.

**Convert is built.** `POST /api/recordings` uploads a raw trace (JSON event
log, HAR, or Playwright trace .zip — whatever the capture step eventually
produces); `POST /api/recordings/[id]/compile` sends it to a
HarnessRouter-configured agent ("Ghost Recording Compiler",
`apps/web/src/lib/recording-compiler.ts`) that proposes a `steps.json`;
`GET /api/recordings/[id]/stream` pushes progress the same way run status is
pushed (`apps/web/src/app/api/runs/[id]/stream`); the proposal is validated
against `@ghost/core/schema/step` before it ever lands in
`Recording.compiledSteps`, then reviewed in the same `WorkflowEditor` used for
hand-authored workflows and published through the existing
`POST /api/workflows` path — the AI proposal never bypasses that trust
pipeline, it only pre-fills the editor. `HR_API_KEY` (server-only,
`cloud/.env`) is required for this to run.

**HarnessRouter is development tooling, not runtime.** It is agent
infrastructure used to build Ghost; nothing a customer executes goes through
it. Compile is an optional authoring convenience that is unavailable when the
key is unset, and production is expected to leave it unset — see
`docs/DEPLOY.md`.

Done: **the provider-neutral `WorkflowCompiler` boundary** (PR #406). The
interface, its `HarnessRouterWorkflowCompiler` implementation, and the
`workflowCompiler()`/`requireWorkflowCompiler()`/`compilerConfigured()`
accessors live in `apps/web/src/lib/compiler/{types,index,harness-router-compiler}.ts`.
`recording-compiler.ts` depends only on `WorkflowCompiler`; HarnessRouter-specific
types stay inside `harness-router-compiler.ts` and never leak into it. Errors
normalize to Ghost-owned `CompilerNotConfiguredError`/`CompilerRequestError`,
and `compiler-optional.test.ts` swaps in a fake for tests. Adding a second
adapter needs no change to the recording routes — only a new file and a branch
in `workflowCompiler()`.

**Capture is decided: a Chrome extension for v1.** A Ghost-hosted remote
browser is the more elegant answer — it records in the same environment the
worker replays in, so environment drift between record and replay stops being
a class of production defect — but running Chrome at scale is infrastructure
engineering, and the extension gets a real capture path in front of users far
sooner. The engine boundary is unchanged either way: Convert needs *a* trace
to exist, not a particular producer, so the extension slots in ahead of the
upload step and `POST /api/recordings` stays as it is. The manual upload form
is scaffolding, not the product.

**Capture is also built, not just decided.** `cloud/apps/extension` is a
working Chrome extension (records clicks/typing/selects/submits/navigation via
accessible role+name, redacts secret-shaped fields at capture, uploads to
`POST /api/agent/recordings` with a revocable bearer token) — see its own
`README.md` for the trust boundary. It lands on `feat/browser-recording-extension`,
not yet merged, so `README.md`'s Phase 2 status line ("capture is next") is
correct as of `master` but stale the moment this branch merges. Update it in
the same PR.

## Worker container: built, but had never actually run

Audited 2026-08-05, prompted by a stale roadmap in this very document (the S3
and four-eyes items above, both already shipped). While verifying what was
*actually* left, `docker build -f apps/worker/Dockerfile .` was run for the
first time since this file's own "no deployment of Ghost exists yet" caveat
in `docs/DEPLOY.md` was written — and it failed, then failed differently, then
crashed on boot. Two independent bugs, both now fixed:

1. The `deps` stage only `COPY`'d `packages/core/package.json`, not
   `packages/core/prisma`. `@ghost/core`'s `postinstall` runs a bare `prisma
   generate`, which resolves the schema at the default `./prisma/schema.prisma`
   path — absent at that point in the build — so `pnpm install` itself failed
   before the `build` stage (which does copy the rest of `packages/core`) ever
   ran. Same root cause silently dropped `tsconfig.base.json`, so
   `apps/worker/tsconfig.json`'s `extends` also failed (non-fatally — tsup
   warned and fell back to its own defaults). Fixed by copying both ahead of
   `pnpm install`.
2. With the image building, the container crashed immediately on boot:
   `Error: Dynamic require of "fs" is not supported`. `tsup.config.ts`'s
   `noExternal: [/^@ghost\//]` bundles `@ghost/core` into the worker's ESM
   output, which transitively pulled in `@prisma/client`'s generated CJS
   runtime (it dynamically `require()`s native query-engine files) — esbuild's
   CJS→ESM interop can't represent that and throws at the first call. Fixed by
   marking `@prisma/client`/`.prisma/client` `external` in `tsup.config.ts`,
   and adding `@prisma/client` as a direct dependency of `@ghost/worker`
   (pnpm's strict linking won't resolve a transitive dep at the worker's own
   require path otherwise).

Verified by rebuilding the image and running it against a real Postgres +
Redis with `docker run --network host`, confirming the `[worker] Ghost worker
started...` log line rather than a crash. `.github/workflows/cloud.yml` now
builds and boot-smoke-tests the image on every PR (`Build worker container
image` / `Smoke-test worker container boots`) — before this, CI's `pnpm build`
only ran `tsup` at the workspace level, which neither exercised the
Dockerfile's own `COPY` list nor ever executed the bundled `dist/index.js`, so
both bugs shipped invisibly across several PRs.

This does not mean a deployment now exists — `docs/DEPLOY.md`'s runbook is
still unexecuted against a real host, and the sign-in trap and domain trap it
documents are still open. It means the one artifact that runbook assumed
worked, and that a first deploy attempt would have discovered the hard way,
now actually does.

## Repo conventions

- Validate from `cloud/`: `pnpm typecheck && pnpm test && pnpm build`.
- Pass `DATABASE_URL` (and a migrated DB) for the worker integration tests.
- API routes / jobs stay org-scoped via `auth()`; keep the trust pipeline intact.
- CI: `.github/workflows/cloud.yml`, path-scoped to `cloud/**`, runs on every
  push/PR against real Postgres + Redis services — see the section above.
- Canonical product docs: `AGENTS.md`, `CLAUDE.md`, `cloud/README.md`. Root
  `README.md` should describe the cloud product; the desktop app is legacy.
