# Ghost Cloud

Ghost is an **AI operator** that learns a business workflow once, then executes
it across the software a company already uses — combining APIs, browser
automation, and (later) desktop automation, with **human approval on sensitive
actions**, **verification of outcomes**, and a **full execution log** for every
run.

This directory holds the cloud SaaS. It is a self-contained pnpm + Turborepo
workspace and does **not** share tooling with the repository root (which still
contains the legacy Rust/Tauri desktop app). Build and run everything from
inside `cloud/`.

## What Ghost does (MVP)

1. **Record** a browser workflow (a user performs the task once).
2. **Convert** the recording into an editable, step-by-step workflow.
3. **Replay** the workflow across browser / API actions.
4. **Require approval** before sensitive actions (send, pay, delete, submit).
5. **Log** every run: per-step status, screenshots, verification, errors.

The engine shape is `Capture → Review → Approve → Execute → Verify → Recover`.
AI proposes; deterministic code executes only approved plans.

## Layout

```
cloud/
  apps/
    web/       Next.js 15 (App Router) — UI + API route handlers        → Vercel
    worker/    Node worker: BullMQ consumers + Playwright execution     → container
  packages/
    core/      Prisma schema/client, Zod workflow-step schema,
               deterministic sensitive-action classifier, shared types
```

- `apps/web` talks to Postgres (via `@ghost/core`) and enqueues jobs to Redis.
- `apps/worker` consumes those jobs and drives Playwright. Playwright needs a
  long-running container, which is why execution lives in the worker and not in
  Vercel serverless functions.
- `packages/core` is the single source of truth for the data model and the
  workflow schema, imported by both apps.

## Prerequisites

- Node >= 20 (repo is developed on Node 22)
- pnpm 10 (`corepack enable` to get the pinned version)
- Docker (for local Postgres + Redis)

## Getting started

```bash
cd cloud
cp .env.example .env            # then edit AUTH_SECRET at minimum
pnpm install                    # runs `prisma generate` via core postinstall
docker compose up -d            # Postgres :5432, Redis :6379
pnpm db:migrate                 # apply the Prisma schema
pnpm dev                        # web on http://localhost:3000 + worker
```

`pnpm dev` runs both apps via Turborepo. To run them separately:

```bash
pnpm --filter @ghost/web dev
pnpm --filter @ghost/worker dev
```

## Smoke test (Phase 0)

1. Open http://localhost:3000 and sign in (the dev-credentials provider accepts
   any email locally; an `Organization` is created on first sign-in).
2. From the dashboard, click **Enqueue test job**.
3. Watch the worker terminal — it logs the consumed no-op job. That proves the
   web ↔ Redis ↔ worker wiring end to end.

## Validation

```bash
pnpm typecheck
pnpm lint
pnpm test
pnpm build
```

## Status

**Phase 0 — running skeleton** (this scaffold): workspace, data model, auth, app
shell, queue wiring, CI. Phases 1–3 (replay + approval + logs, then recording,
then hardening) are specified in the project plan and build on this skeleton.
