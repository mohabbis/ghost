# Ghost Native macOS Implementation Package

This additive change introduces:

- a SwiftUI macOS 13+ application under `apps/macos`;
- a versioned local Rust bridge binary under `src-tauri/src/bin`;
- a complete Organizer workflow through receipt and undo;
- signed plan-bound approval tokens and server-side replanning;
- local-vault lock enforcement;
- a project-local build/run entrypoint and Codex Run action;
- operational architecture documentation.

Validation performed in the construction environment:

- SwiftPM manifest resolution with `swift package dump-package`;
- shell syntax validation with `bash -n script/build_and_run.sh`;
- static contract review against the current public Rust modules on `master`.

This environment is Linux and has no Rust toolchain or macOS SDK, so it cannot
truthfully validate SwiftUI compilation, Rust compilation, app bundling, signing,
or launch behavior. Run these on macOS before merge:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
swift build --package-path apps/macos
./script/build_and_run.sh --verify
```
