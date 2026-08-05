# Ghost worker — the BullMQ consumer that drives Playwright.
#
# Build from the `cloud/` directory, not from here:
#   docker build -f apps/worker/Dockerfile -t ghost-worker .
# The worker imports @ghost/core through the pnpm workspace, so the build
# context has to include the whole workspace.
#
# Base image is Microsoft's Playwright image at the *same version as the
# playwright dependency*. That is deliberate: Playwright looks for the exact
# browser revision its own version pins, and a mismatch fails at launch with
# "Executable doesn't exist at .../chromium_headless_shell-<rev>/..." — the
# failure that made every run die at step 0 before it was fixed. Bump this tag
# and the playwright dependency together, never one alone.
FROM mcr.microsoft.com/playwright:v1.62.0-noble AS base

ENV PNPM_HOME=/pnpm
ENV PATH=$PNPM_HOME:$PATH
# PLAYWRIGHT_BROWSERS_PATH is deliberately left unset. `discoverChromium()` in
# browser/chromium.ts only looks when that variable is set, so leaving it alone
# means it no-ops and Playwright uses the browsers baked into this image — the
# revision matches by construction rather than by configuration.
ENV NODE_ENV=production
RUN corepack enable && corepack prepare pnpm@10.33.0 --activate

WORKDIR /app

# ---- dependencies -----------------------------------------------------------
FROM base AS deps
# Manifests first so a code-only change does not re-resolve the dependency graph.
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml turbo.json tsconfig.base.json ./
COPY packages/core/package.json packages/core/
# packages/core's postinstall runs a bare `prisma generate`, which resolves
# the schema at the default `./prisma/schema.prisma` path relative to the
# package. Without it here, `pnpm install` fails inside this stage before the
# build stage (which copies the rest of packages/core) ever runs.
COPY packages/core/prisma packages/core/prisma
COPY apps/worker/package.json apps/worker/
RUN pnpm install --frozen-lockfile --filter @ghost/worker...

# ---- build ------------------------------------------------------------------
FROM deps AS build
COPY packages/core packages/core
COPY apps/worker apps/worker
# Prisma Client is generated, not vendored; without this the worker cannot
# reach the database.
RUN pnpm --filter @ghost/core exec prisma generate \
 && pnpm --filter @ghost/worker build

# ---- runtime ----------------------------------------------------------------
FROM base AS runtime
COPY --from=build /app /app
WORKDIR /app/apps/worker

# The image ships no default DATABASE_URL, REDIS_URL or GHOST_SESSION_KEY.
# Failing loudly on a missing variable is better than silently connecting
# somewhere unintended — see cloud/docs/DEPLOY.md for the full set.
#
# Migrations are NOT run here. Several workers boot together and would race the
# same migration; `pnpm db:migrate:deploy` belongs in a release step that runs
# exactly once. See DEPLOY.md.
#
# The Playwright image provides an unprivileged `pwuser`; Chromium should not
# run as root.
USER pwuser

CMD ["node", "dist/index.js"]
