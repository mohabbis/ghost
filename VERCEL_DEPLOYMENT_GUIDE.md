# Vercel notes

## Marketing site (`public/`)

Deployed by `deploy-website.yml` when `public/**` changes. Env needed on the
Vercel project: `VERCEL_TOKEN` / org / project IDs in GitHub secrets.

## Cloud web app (`cloud/apps/web`)

Can be deployed as a separate Vercel project pointing at the Turborepo app.
Required env mirrors `cloud/.env.example` (`AUTH_SECRET`, `DATABASE_URL`,
`REDIS_URL`, `AUTH_URL`, artifact config). The **worker cannot run on Vercel
serverless** — Playwright needs a persistent container.

Do not put production secrets in the repo.
