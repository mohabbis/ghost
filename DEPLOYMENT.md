# Deployment

## Ghost Cloud (active)

Stack: Next.js web (Vercel or Node) · Node worker (container) · Postgres · Redis · Playwright.

Local:

```bash
cd cloud
cp .env.example .env   # AUTH_SECRET, GHOST_ARTIFACT_DIR, APP_URL, DATABASE_URL, REDIS_URL
pnpm install
docker compose up -d
pnpm db:migrate
pnpm --filter @ghost/worker exec playwright install chromium
pnpm dev
```

Production sketch:

- **Web** — `cloud/apps/web` on Vercel (or any Node 20+ host); set Auth.js + DB + Redis env
- **Worker** — long-running container with Chromium; same `DATABASE_URL` / `REDIS_URL` / artifact store
- **Postgres / Redis** — managed services
- **Artifacts** — S3-compatible in prod; disk only for local

Cloud CI workflow is staged at `cloud/ci/cloud.yml` until installed under
`.github/workflows/`.

**Live today:** the production domain **`ghost.muharafiq.com`** points at the
`ghost-app` Vercel project with its Root Directory set to `cloud/apps/web` — it
serves the cloud SaaS app directly (deployed manually today via
`vercel --prod --scope muharafiq --cwd cloud/apps/web`, not by a checked-in CI
workflow). This is a deliberate change from the domain's original use as the
static marketing site — see "Marketing site" below.

Details: `cloud/README.md`, `cloud/docs/CURSOR_HANDOFF.md`.

## Marketing site (currently not deployed anywhere)

`public/` is a separate static site (vanilla JS, ships the legacy desktop
Ghost.dmg/Ghost_Setup.exe installers) for the superseded desktop product, not
the cloud SaaS. `.github/workflows/deploy-website.yml` still exists to deploy
it, but it targets the same `ghost-app` Vercel project that now serves
`cloud/apps/web` above — because Vercel's Root Directory override applies
regardless of which files actually changed, running this workflow today would
redeploy the cloud app again, not `public/`, despite its build log claiming
otherwise. Its automatic trigger is disabled for that reason (see the workflow
file). Before re-enabling it: either point it at a separate Vercel
project/domain for the legacy site, or retire `public/` outright if the legacy
desktop product no longer needs a public download page.

## Legacy desktop

Installer builds for the Rust/Tauri app: see `RELEASING.md` and `docs/legacy/`.
Not part of the commercial cloud deploy path.
