# Deploying Ghost Cloud

Ghost is four processes: a Next.js web app, a worker that drives a real browser,
Postgres, and Redis. Plus object storage once web and worker are on separate
hosts.

**Update:** `apps/web` has now actually been deployed — Vercel project
`ghost-app`, Neon Postgres, Upstash Redis, migrations applied, `/signin`
verified live. The bugs that first deploy found (a stale root `vercel.json`
silently winning over the app's own build config, the CLI uploading untracked
worktree directories, Vercel's own SSO wall blocking every page) are folded
into the steps below rather than left as a separate war story. **The worker
has not been deployed** — no container host is wired up yet, and object
storage was deliberately skipped (screenshots fall back to local disk, which
does not survive being served from a separate worker host). Treat both of
those as still-open parts of a first real deploy.

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
| `ARTIFACT_RETENTION_DAYS` | | ○ | defaults to 90; the `purge-artifacts` job (scheduled by the worker itself, see `apps/worker/src/index.ts`) deletes a run's screenshots once it ended this many days ago |
| `SENTRY_DSN` | | ○ | **worker only.** Error tracking stays off without it, matching `HR_API_KEY`/`S3_BUCKET` (see `@ghost/core/sentry`). `apps/web` is not wired to this — `@sentry/node`'s auto-instrumentation cannot be webpack-bundled (tried, broke `pnpm build`); wiring web needs `@sentry/nextjs` via `npx @sentry/wizard@latest -i nextjs` against a real Sentry project |

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

## Provisioning checklist — accounts and dashboards only you can create

Nothing above can be automated further: each step below needs a real account,
billing, or a browser session with your organization's credentials, so this
codebase can prepare the code but not take the step for you. Grouped by
provider rather than by step, since you likely do these once, in whatever
order your accounts allow.

**Postgres + Redis** — any managed provider works; nothing here is
provider-specific. Note the connection strings for `DATABASE_URL` and
`REDIS_URL`. If the provider requires TLS, confirm the Prisma/ioredis
connection strings encode that (`?sslmode=require`, `rediss://`) — untested
here, since no deployment exists yet.

**Object storage (S3-compatible)** — AWS S3, Cloudflare R2, or Backblaze B2 all
work (`S3_ENDPOINT` selects a non-AWS one). Create one bucket, one set of
credentials, and scope the credentials' policy to that bucket only rather than
reusing an account-wide key — a credential leaked from either process should
not be able to reach anything else in the account. A minimal AWS IAM policy:
```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:ListBucket"],
    "Resource": ["arn:aws:s3:::your-bucket-name", "arn:aws:s3:::your-bucket-name/*"]
  }]
}
```
Set the same `S3_*` variables on **both** the Vercel project and the container
host — see "Object storage is not optional here" above for what happens if
only one side gets them.

**GitHub OAuth app** (github.com → Settings → Developer settings → OAuth Apps
→ New OAuth App):
- Homepage URL: your web app's eventual domain (see the Vercel project below).
- Authorization callback URL: `https://<your-app-domain>/api/auth/callback/github`
  — exact match, including the path; NextAuth rejects a mismatch rather than
  redirecting somewhere unexpected.
- Note the Client ID and generate a Client Secret; these become
  `AUTH_GITHUB_ID` / `AUTH_GITHUB_SECRET`. See "The sign-in trap" above for
  what happens if you skip this.

**Vercel project for `cloud/apps/web`** — this is "The domain trap" above made
concrete:
1. Create a **new, second** Vercel project. Do not add `cloud/apps/web` to
   whatever project already serves the repo root's `public/` marketing site —
   they cannot share one project or one `vercel.json`.
2. Set its **Root Directory** to `cloud/apps/web` in Project Settings.
3. **`cloud/apps/web/vercel.json` must exist**, at minimum `{"framework":
   "nextjs"}`. Contrary to what this file previously said here: with the repo
   root's own `vercel.json` present (the marketing site's static-build config),
   an empty Root Directory at build time falls back to that root config
   instead of auto-detecting Next.js — even though Root Directory is correctly
   set on the project and even though the uploaded source is scoped correctly.
   Vercel's own build output prints "The vercel.json file should be inside of
   the provided root directory" as a warning, not a hard error, and then uses
   it anyway. Confirmed by deploying without one first: the build ran the
   marketing site's `echo` install/build commands against `cloud/apps/web`'s
   uploaded files and failed looking for a `public/` output directory that
   doesn't exist there. A local `vercel.json` closes the gap outright.
4. If deploying via the CLI rather than Git integration, also add a
   `.vercelignore` at the **repo root** excluding any local worktree/build
   directories that sit untracked next to the actual source (`.worktrees/`,
   `.wt/`, `.turbo/` in this repo, `src-tauri/target/`, `dist/`) — `vercel
   deploy` uploads the working directory as-is, not `git ls-files`, so
   untracked local dev artifacts get swept in too. Without it, a deploy from
   this repo's root uploaded 57,896 files instead of ~700.
5. Give it its own hostname (e.g. `app.<your-domain>`, distinct from the
   marketing site's).
6. Set every `web`-column environment variable from the table above on this
   project (`DATABASE_URL`, `REDIS_URL`, `AUTH_SECRET`, `AUTH_GITHUB_ID`/
   `AUTH_GITHUB_SECRET`, the `S3_*` set, `APP_URL`).
7. **Disable Vercel's own Deployment Protection (SSO/Vercel Authentication)**
   for this project, or it gates every page — including `/signin` — behind a
   Vercel-account login wall on top of Ghost's own auth, blocking real users
   entirely. New projects on a team plan often have this on by default for
   `*.vercel.app` URLs.

**Container host for the worker** (Fly.io, Railway, Render, or any host that
runs a Docker image — pick one; none of this repo's code prefers one over
another):
1. Point it at `apps/worker/Dockerfile`, built with `cloud/` as the build
   context (see step 5 above — the workspace layout requires this).
2. Set every `worker`-column environment variable from the table above,
   **plus** `GHOST_SESSION_KEY` (worker-only) and, optionally,
   `ARTIFACT_RETENTION_DAYS`/`SENTRY_DSN`.
3. Most of these hosts expect a process that stays up and reads its port/health
   check from an env var they inject — the worker has no HTTP server and needs
   none (it is a queue consumer), so skip any "web service" health-check
   requirement the host's UI assumes by default and configure it as a
   background worker / long-running process instead.

**Sentry (optional)** — create a project, generate a DSN, and set
`SENTRY_DSN` on the container host only; this alone gives the worker error
tracking, since `apps/worker` is already wired against `@ghost/core/sentry`
(see that file for why the wrapper doesn't try to also cover `apps/web`).
Wiring `apps/web` needs `@sentry/nextjs`, not this env var — run
`npx @sentry/wizard@latest -i nextjs` against the same Sentry project once the
Vercel project above exists, since the wizard needs a real project to
configure against and writes files (`instrumentation-client.ts`,
`sentry.server.config.ts`, a `next.config.ts` wrapper) this repo does not ship.

## Rolling back

Application rollbacks are ordinary redeploys. Migrations are not: `migrate
deploy` only rolls forward, so a schema change that has to be undone needs a new
migration. Prefer additive changes.

## HarnessRouter is development tooling — leave it unset in production

HarnessRouter is agent infrastructure used to *build* Ghost. It is not part of
what Ghost *runs*. Nothing a customer executes — no workflow, no step, no
approval, no verification — passes through it, and production must boot and
operate with `HR_API_KEY` absent.

It backs exactly one optional authoring convenience: recording compile
(`cloud/apps/web/src/lib/recording-compiler.ts`) turns an uploaded trace into
proposed steps. That proposal is never executed; a human reviews it in the
editor and publishes through the normal `POST /api/workflows` path. With the
key unset the feature is unavailable and nothing else changes.

Two reasons to keep it that way, both of which are why the boundary exists
rather than being a matter of taste:

**A trace is customer data.** It is a recording of someone doing real work in
their own systems. HAR files and Playwright traces carry request and response
bodies, `Cookie` and `Authorization` headers, and everything typed. The
compiler agent is instructed never to copy a captured secret into its output —
but that protects the *proposal*, not the *transfer*. The trace is uploaded
whole, unredacted, to a third party your customer never contracted with.

**One key would serve every tenant.** `HR_API_KEY` is process-wide, so every
organization's traces and compile sessions would share one HarnessRouter
Workspace. Ghost's own routes are org-scoped and refuse cross-tenant access — a
recording id from another org returns 404 on detail, compile, continue, cancel
and stream, and claiming another org's recording while publishing is refused —
but that is Ghost enforcing isolation on its own surface, not isolation inside
the vendor. Anyone holding the key could enumerate every tenant's sessions
there.

Earlier revisions of this document offered per-organization HarnessRouter
credentials as the fix. That was the wrong frame: it would have addressed the
second point, not the first, and bought a provisioning system for a dependency
that should not be in the runtime at all.

**A trace is customer data.** It is the recording of someone performing real
work in their own systems, so it can contain URLs, record identifiers,
customer names, and anything else that was on screen. The compiler agent is
instructed never to copy a captured secret into a step (passwords, OTPs, card
numbers become placeholders), but that is an instruction to a model, not an
enforced guarantee — the trace itself is uploaded whole. Do not point this at
a system whose screens carry data you are unwilling to send to HarnessRouter.

## What this does not cover

No autoscaling, no multi-region, no backup policy, no log aggregation, no
alerting, and no rate limiting on the agent API. Also worth knowing before
putting real customer systems behind this:

**Separation of duties is available but off by default.** Turn on "2nd approver"
per workflow and the person who triggered a run can no longer approve its gates —
only reject them. It is opt-in because organizations are created single-member on
first sign-in, so switching it on in a one-person org leaves nobody able to
approve and runs stop at their first gate.

**Approval is still not role-restricted.** Membership, invitations, and roles
(`OWNER`/`ADMIN`/`MEMBER`) are real and tested — `app/api/settings/members`,
`app/api/invitations`, and `packages/core/src/roles.ts`'s `isOrgAdmin` already
gate admin-only actions like revoking a colleague's credential. What is
missing is narrower: `POST /api/runs/[id]/approvals/[stepIndex]` checks org
membership and `requireSeparateApprover` (requester ≠ approver) but reads
`Membership.role` not at all, so any member — `MEMBER` included — can approve
a run they did not start. Fixing it is a role check at one call site, not a
membership model that needs building first.

Agents genuinely cannot approve, and that is enforced: no POST handler on
`/api/agent/approvals`, an explicit 403, and a forbidden-tools list.
