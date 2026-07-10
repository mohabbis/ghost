# Ghost — build notes (macOS)

## Quick path

```bash
cd /path/to/ghost
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo build --manifest-path src-tauri/Cargo.toml
./src-tauri/target/debug/ghost
```

Or with the Tauri CLI (hot reload for `src/`):

```bash
cargo tauri dev
```

## Do not restore hang config

Do **not** add `src-tauri/.cargo/config.toml` with aggressive `codegen-units = 1` /
LTO overrides for local debug builds — it has hung compiles on Apple Silicon.
Release CI can keep its own profile; local debug should stay default.

## First-run dogfood checklist

1. Onboarding → optional local unlock password (Skip is OK).
2. **Organizer** → Browse / prefilled Downloads → Scan → Approve → Undo.
3. **Guard Desk** → pick preset → Scan → Approve plan → Auto-fill POS.
4. Recording still needs Accessibility + Input Monitoring; Organizer does not.
