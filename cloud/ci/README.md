# CI for Ghost Cloud

> **`cloud/` has no CI today.** Nothing in this directory runs. The only
> workflows GitHub executes are `rust.yml`, `security.yml`, `release.yml` and
> `deploy-website.yml` — none of which touch `cloud/`.
>
> `rust.yml` is not path-scoped, so it runs on *every* pull request. A PR that
> changes only `cloud/` still shows ~22 green checks, and **not one of them
> compiles, typechecks or tests a line of the changed code**. That is the single
> most misleading thing about this repository right now.

## Activating it

`cloud.yml` cannot live in `.github/workflows/` until someone with the
`workflow` OAuth scope moves it. Automation tokens (including the one that
wrote this file) are refused by GitHub with:

```
refusing to allow an OAuth App to create or update workflow
`.github/workflows/cloud.yml` without `workflow` scope
```

A maintainer signed in normally can do it in one command:

```bash
git mv cloud/ci/cloud.yml .github/workflows/cloud.yml
git commit -m "ci: run the cloud workflow"
git push
```

Delete this README at the same time — once the workflow is in place, it is
describing a problem that no longer exists.

## What it runs

Scoped to `cloud/**` and its own path, so it never fires on Rust-only changes
and never collides with `rust.yml`. From `cloud/`: `pnpm install
--frozen-lockfile`, `db:validate`, `db:migrate:deploy`, `typecheck`, `test`,
`build`.

## Why it starts real services

Postgres and Redis are service containers, not placeholder environment
variables. About 90 of the suite's 220 tests are gated on
`Boolean(process.env.DATABASE_URL)`, which makes a URL pointing at nothing the
worst of the three options:

| `DATABASE_URL` | Guard reads | Result |
| --- | --- | --- |
| unset | no database | ~90 tests silently skip; the check is green and covers none of the engine |
| set, nothing listening | database available | tests run and fail on connection refused |
| set, Postgres running | database available | the engine is actually tested |

The earlier draft of this workflow took the middle option, so moving it into
place would have gone red immediately.

Two details that cost time when they were wrong:

- The database user must be **`ghost`**, not `postgres`. The wrong one fails
  with "User was denied access on the database", which reads like a missing
  migration and sends you looking in the wrong place entirely.
- `prisma migrate deploy`, not `migrate dev` — the latter is interactive and
  will offer to reset the database. That is what `db:migrate:deploy` is for.

## Verification

The exact step sequence was run locally against a freshly created, empty
database before this file was committed: `db:validate` → `db:migrate:deploy`
(17 tables from nothing) → `typecheck` 5/5 → `test` **220 passing** → `build`
4/4. Worth re-checking that the worker suite reports 90 tests rather than 45 the
first time this runs for real; 45 means the database is not being reached and
the guard has quietly disarmed itself again.
