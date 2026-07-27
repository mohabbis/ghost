# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in the Ghost repository.

## Quick Reference

### Common Commands
- **Format code**: `make fmt` or `cargo fmt --manifest-path src-tauri/Cargo.toml`
- **Check formatting**: `make fmt-check` or `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- **Lint**: `make clippy` or `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- **Build (no bundle)**: `make build` or `cargo tauri build --no-bundle`
- **Development server**: `make dev` or `cargo tauri dev`
- **Run all tests**: `make test` or `cargo test --manifest-path src-tauri/Cargo.toml`
- **Run a single backend test**: `cargo test --manifest-path src-tauri/Cargo.toml <test_name>`
- **Run a single backend test file**: `cargo test --manifest-path src-tauri/Cargo.toml --test <file_stem>`
- **Run frontend test**: `node --test src/compression-review.test.mjs`
- **CI checks**: `make ci` (format check, clippy, test)

### Project Structure
- `src/` - Tauri frontend (Vite, TypeScript)
- `src-tauri/` - Rust backend
  - `commands/` - Tauri command implementations
  - `core/` - Core logic (execution, compression, OCR, etc.)
  - `organizer/` - File organizer logic
  - `runtime/` - Action Plan runtime (Ghost 2.0)
  - `mcp/` - Model Context Protocol implementation
- `apps/macos/` - Native macOS app (SwiftUI)
- `public/` - Static marketing site (deployed to Vercel)
- `docs/` - Documentation

## Product Identity & Trust Model

Ghost is a local-first desktop automation product for macOS and Windows, positioned for finance and operations teams. Its core promise is:

```
Record -> Inspect -> Approve -> Replay -> Audit -> Undo
```

Ghost must not be presented as a generic autonomous agent. Every integration is additive to the trust pipeline, never a way around it.

### Current Wedge: Ghost Organizer
Prioritize Organizer before broad automation:
```
Select folder -> Scan -> Propose plan -> Review -> Approve -> Move/Rename -> Audit -> Undo
```
Required behavior:
- Preview every filesystem mutation
- Deny silent delete and silent overwrite
- Detect conflicts
- Require approval before mutation
- Write audit events
- Write undo data before reversible operations

## Architecture Direction

- **Keep Rust/Tauri**. Do not rewrite the whole product before proving the wedge.
- **Build in this order**: 
  1. Ghost Organizer (safe file/folder cleanup, classification, naming, moving, conflict detection, preview, audit, undo)
  2. Ghost Routines (explicit recorded routines, deterministic event compression, semantic replay with coordinates as fallback)
  3. Ghost Intelligence (suggestion-only planning, classification, explanation, routine detection)
- **Version**: The source tree is at Ghost 2.0.4, but the latest public release is v2.0.3. Reference v2.0.3 for downloadable builds.

## Validation

Use the relevant checks before claiming work is complete:
```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm ci
cargo tauri build --no-bundle
```
For experimental features, add `--features experimental` to Cargo commands.

### Running Tests
- **Backend (Rust)**: Use `cargo test` as shown above.
- **Frontend**: Run `node --test src/compression-review.test.mjs` for the existing test.

## Command Surface Expectations

Every Tauri command must document whether it touches:
- files
- OS input
- screenshots/screen contents
- network
- authentication/secrets
- app/window state

Experimental commands are gated behind the `experimental` Cargo feature. The frontend hides experimental UI unless `is_experimental_enabled` returns true.

## Response Format

When finishing a task, report:
- files changed
- commit SHA if applicable
- validation performed
- risks or follow-up work

Do not claim a build, release, signing, notarization, or CI result unless it actually happened.