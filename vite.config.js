import { defineConfig } from "vite";

// Ghost desktop app frontend (the `src/` directory). This is NOT the marketing
// site under `public/` at the repo root — that stays a static Vercel deploy.
//
// Tauri's webview is a single modern engine (WKWebView / WebView2), so we target
// evergreen output and, crucially, disable Vite's inline modulepreload polyfill:
// the app ships a strict `script-src 'self'` CSP (see src-tauri/tauri.conf.json)
// that would reject the injected inline <script>. Production output is therefore
// only external, hashed ES modules — CSP-clean.
export default defineConfig({
  root: "src",
  // src/public/ holds pass-through static assets (favicon, /assets/*). Absolute
  // URLs like /favicon.svg and /assets/ghost-mark.svg resolve from here verbatim.
  publicDir: "public",
  // Tauri loads over the custom `tauri://` / asset protocol, so emit relative URLs.
  base: "./",
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: ["es2022", "safari14"],
    // Keep the bundle CSP-clean: no inline preload polyfill script.
    modulePreload: { polyfill: false },
    // Vite 8 bundles with rolldown and no longer ships esbuild; use the default
    // (oxc) minifier rather than pulling esbuild back in as a separate dep.
    minify: true,
    sourcemap: false,
  },
});
