import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["src/index.ts"],
  format: ["esm"],
  target: "node20",
  clean: true,
  // Bundle the workspace package (it ships TS source via subpath exports) so the
  // built worker runs on plain Node.
  noExternal: [/^@ghost\//],
});
