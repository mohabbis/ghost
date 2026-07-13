# GhostAXHelper sidecars (macOS)

Tauri `externalBin` expects architecture-specific binaries here when bundling on macOS:

- `ghost-ax-helper-aarch64-apple-darwin`
- `ghost-ax-helper-x86_64-apple-darwin`

Build on macOS:

```bash
make ax-helper
```

Release CI and local DMG builds pass `--config '{"bundle":{"externalBin":["bin/ghost-ax-helper"]}}'`
so Linux/Windows CI is unaffected. These files are gitignored; do not commit built binaries.
