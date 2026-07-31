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

Phase 0 + Phase 1.1 + Phase 1.2 are **merged on `master`**. The DB-backed
demo loop has been executed end-to-end:

```text
sign-in → create demo workflow → run → AWAITING_APPROVAL on Submit
  → Approve → prefix restore → submit → verify → SUCCEEDED
```

Reject path fails the run without submitting.

### Bug fixed: resume after approval

Ephemeral Chromium is closed when the worker halts at a gate. On resume the
job used to start on `about:blank` and time out looking for "Submit order".
`restoreBrowserPrefix` in `apps/worker/src/browser/driver.ts` replays
`steps[0..cursor)` before continuing. Covered by:

- `apps/worker/src/browser/driver.test.ts` (hermetic restore)
- `apps/worker/src/jobs/runWorkflow.test.ts` (Postgres gate → approve → succeed;
  reject → fail) — runs when `DATABASE_URL` is set, otherwise skipped

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

## Remaining Phase 1 work (priority)

1. **Typed step editor** (`workflows/new` + `[id]`) — author steps, not only
   the demo seed. Validate with `@ghost/core` `workflowStep` Zod schema. This is
   also the prerequisite for n8n-style pinned data / partial re-execution.
2. **Credential hardening** — optional expiry and organization-admin inventory/revocation.
3. **SSE/WebSocket** for the timeline (nice-to-have; polling works).
4. **S3 serve path** — disk store works in dev; wire presigned URLs when S3 is on.
   Note the artifact route is now a positive allow-list (`step-`/`restore-` PNGs
   only); keep it that way, because the same prefix holds encrypted session
   blobs that must never be served.
5. **Compensation / undo handlers** — the journal makes the saga pattern
   tractable: walk it backwards invoking each executed step's compensator.
6. **Four-eyes approval** — needs RBAC first; `Role` is stored and never enforced,
   and orgs are auto-created single-member on first sign-in.

## Phase 2

Recording → editable workflow. Open decision: server-side remote cloud browser
(recommended) vs Chrome extension. Engine boundary is clean either way.

## Repo conventions

- Validate from `cloud/`: `pnpm typecheck && pnpm test && pnpm build`.
- Pass `DATABASE_URL` (and a migrated DB) for the worker integration tests.
- API routes / jobs stay org-scoped via `auth()`; keep the trust pipeline intact.
- CI: `cloud/ci/cloud.yml` is staged outside `.github/workflows/` until a token
  with `workflow` scope can move it (see `cloud/ci/README.md`).
- Canonical product docs: `AGENTS.md`, `CLAUDE.md`, `cloud/README.md`. Root
  `README.md` should describe the cloud product; the desktop app is legacy.
