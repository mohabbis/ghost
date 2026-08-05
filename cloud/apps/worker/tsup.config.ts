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
  // node_modules instead.
  external: ["@prisma/client", ".prisma/client"],
});
