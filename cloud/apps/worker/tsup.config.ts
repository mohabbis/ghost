import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["src/index.ts"],
  format: ["esm"],
  target: "node20",
  clean: true,
  // Bundle the workspace package (it ships TS source via subpath exports) so the
  // built worker runs on plain Node.
  noExternal: [/^@ghost\//],
  // @prisma/client's generated runtime is CJS that dynamically `require()`s
  // native query-engine files by path. Inlining it into the ESM bundle makes
  // esbuild rewrite that into a `require` shim that throws "Dynamic require of
  // 'fs' is not supported" the instant the worker boots. Keep it (and the
  // generated `.prisma/client`) external so Node loads it normally from
  // node_modules instead. `@sentry/node` does its own runtime instrumentation
  // (dynamic requires for auto-instrumenting Node built-ins, optional native
  // profiling modules) for the same reason — Sentry's own docs say not to
  // bundle it, so it gets the same treatment rather than waiting to discover
  // the same crash a second time. `dotenv` (pulled in transitively through
  // `@ghost/core/env`, which is `noExternal` above) hits the identical
  // "Dynamic require of 'fs'" crash via its own internal `require("fs")`.
  // `@vercel/blob` (used by the artifact store's Vercel Blob backend) pulls in
  // `@vercel/oidc` -> `jose`'s CJS build, which does the same dynamic
  // `require("buffer")` — same crash, one dependency further down. `pnpm
  // install --filter @ghost/worker...` in the Dockerfile puts all of these in
  // node_modules regardless, so externalizing costs nothing at runtime.
  external: ["@prisma/client", ".prisma/client", "@sentry/node", "dotenv", "@vercel/blob"],
});
