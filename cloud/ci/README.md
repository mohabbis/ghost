# CI for Ghost Cloud

`cloud.yml` is the GitHub Actions workflow for the `cloud/` SaaS. It lives here
instead of `.github/workflows/` because the automation token that opened this
branch lacks the `workflow` OAuth scope, so it cannot create or update files
under `.github/workflows/`.

**To activate it**, a maintainer with `workflow` scope should move it into place:

```bash
git mv cloud/ci/cloud.yml .github/workflows/cloud.yml
git commit -m "ci: add cloud workflow"
git push
```

The workflow is scoped to `cloud/**` (and its own path), so it never runs on
Rust-only changes and never collides with `rust.yml`. It runs, from `cloud/`:
`pnpm install --frozen-lockfile`, `prisma validate`, `typecheck`, unit tests,
and `build`.
