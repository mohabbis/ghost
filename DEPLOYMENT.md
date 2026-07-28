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

Details: `cloud/README.md`, `cloud/docs/CURSOR_HANDOFF.md`.

## Marketing site

Static files in `public/` deploy via `.github/workflows/deploy-website.yml` to Vercel.
This is the public marketing surface — keep it aligned with the cloud product.

## Legacy desktop

Installer builds for the Rust/Tauri app: see `RELEASING.md` and `docs/legacy/`.
Not part of the commercial cloud deploy path.
