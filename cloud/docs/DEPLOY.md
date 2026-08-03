# Deploying Ghost Cloud

Ghost is four processes: a Next.js web app, a worker that drives a real browser,
Postgres, and Redis. Plus object storage once web and worker are on separate
hosts.

This document is a runbook, not a record — **no deployment of Ghost exists
yet**, and nothing here has been executed against a real host. It was written
alongside the code that makes it possible, and the container image has not been
built (Docker was unavailable in the environment that wrote it). Treat the first
deploy as the test of this file.

## Shape

```text
  Vercel  ──────────────►  Postgres (managed)
  web app                    ▲
     │                       │
     │  BullMQ via Redis     │
     ▼                       │
  container host  ───────────┘
  worker + Chromium
     │
     └────► S3-compatible bucket (screenshots, encrypted session blobs)
```

The worker cannot go on Vercel: it is a long-lived queue consumer that holds a
browser open for the length of a run. It needs a container host — Fly, Railway,
Render, or any box that runs Docker.

## Object storage is not optional here

With web and worker on separate hosts there is no shared filesystem, so
`S3_BUCKET` and credentials must be set **for both**. Without them each process
independently falls back to its own local disk: the worker writes screenshots to
a container that the web app cannot read, every image 404s, and the person at an
approval gate is deciding with no evidence in front of them.

Any S3-compatible store works — set `S3_ENDPOINT` for Cloudflare R2, Backblaze
B2, or MinIO.

## Environment

| Variable | web | worker | If unset |
| --- | :-: | :-: | --- |
| `DATABASE_URL` | ● | ● | nothing works |
| `REDIS_URL` | ● | ● | runs queue and never execute |
| `AUTH_SECRET` | ● | | sessions cannot be signed |
| `AUTH_GITHUB_ID` / `AUTH_GITHUB_SECRET` | ● | | **no way to sign in at all** — see below |
| `S3_BUCKET`, `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY` | ● | ● | silent fallback to local disk; artifacts lost |
| `S3_REGION`, `S3_ENDPOINT` | ○ | ○ | needed for non-AWS S3-compatible stores |
| `GHOST_SESSION_KEY` | | ● | gated runs cannot resume after approval |
| `APP_URL` | ● | ● | the demo fixture is unreachable from the worker |
| `NEXT_PUBLIC_SOURCE_URL` | ○ | | AGPL §13 link points at upstream, not your fork |

`GHOST_SESSION_KEY` is **worker-only** by design. It decrypts captured browser
sessions — live cookies for the customer's systems — and the web app has no
reason to hold that power.

### The sign-in trap

`apps/web/src/app/signin/page.tsx` shows the dev email form only when
`NODE_ENV !== "production"`, and the GitHub button only when both GitHub
variables are set. In production with neither configured, the page renders with
**no way to sign in**. Configure the GitHub OAuth app before the first deploy,
with its callback at `https://<your-app-domain>/api/auth/callback/github`.

### The domain trap

The repository's root `vercel.json` builds `public/` — the static marketing site
on `ghost.muharafiq.com`. The app cannot share that project. Create a **second**
Vercel project with root directory `cloud/apps/web` and give it its own
hostname, e.g. `app.ghost.muharafiq.com`.

## Order of operations

1. Provision Postgres, Redis and a bucket.
2. Create the GitHub OAuth app; note the id and secret.
3. Generate secrets: `openssl rand -base64 32` for `AUTH_SECRET`, and again for
   `GHOST_SESSION_KEY`.
4. **Run migrations once**, from anywhere with `DATABASE_URL` set:
   ```bash
   cd cloud && pnpm db:migrate:deploy
   ```
   This is a release step, not a boot step. Two workers starting together would
   race the same migration.
5. Deploy the worker:
   ```bash
   cd cloud
   docker build -f apps/worker/Dockerfile -t ghost-worker .
   ```
   Build from `cloud/`, not from `apps/worker/` — the worker imports
   `@ghost/core` through the workspace, so the context needs both.
6. Deploy the web app to its Vercel project.
7. Sign in, create the demo workflow, run it, approve it. **The screenshot on
   the approval must render** — that is the one check that proves object storage
   is wired correctly on both sides, and it is exactly what a disk fallback
   breaks.

## Rolling back

Application rollbacks are ordinary redeploys. Migrations are not: `migrate
deploy` only rolls forward, so a schema change that has to be undone needs a new
migration. Prefer additive changes.

## What this does not cover

No autoscaling, no multi-region, no backup policy, no log aggregation, no
alerting, and no rate limiting on the agent API. Also worth knowing before
putting real customer systems behind this:

**Separation of duties is available but off by default.** Turn on "2nd approver"
per workflow and the person who triggered a run can no longer approve its gates —
only reject them. It is opt-in because organizations are created single-member on
first sign-in, so switching it on in a one-person org leaves nobody able to
approve and runs stop at their first gate.

**Approval is still not role-restricted.** `Membership.role` is stored and read
by nothing; any member can approve any run they did not start. That gap is
blocked on member management rather than on effort — there is no invite flow, so
every organization has exactly one person and a role check would enforce a
distinction the system cannot yet express.

Agents genuinely cannot approve, and that is enforced: no POST handler on
`/api/agent/approvals`, an explicit 403, and a forbidden-tools list.
