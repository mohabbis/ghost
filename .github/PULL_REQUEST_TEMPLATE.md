<!--
Ghost PR template. Fill in each section; delete guidance comments as you go.
Keep changes scoped and preserve the trust model (see AGENTS.md / CLAUDE.md).
-->

## Summary

<!-- What does this change do, and why? Link any related issue or doc. -->

## Trust & safety

<!-- For any command or behavior change, note what it touches. Delete rows that
do not apply; "none" is a valid answer. -->

- Touches files / filesystem mutation:
- Touches OS input (keyboard/pointer capture or replay):
- Touches screenshots / screen contents:
- Touches network:
- Touches authentication / secrets:
- Touches app / window state:

For mutating operations, confirm the pipeline still holds
(Intent → Plan → Policy check → Approval → Execution → Audit → Undo):

- [ ] Risky actions remain deny-by-default
- [ ] Reversible mutations write undo data before executing
- [ ] No silent delete or silent overwrite
- [ ] New Tauri commands have a module and a risk class
- [ ] Experimental features stay gated (`--features experimental`) or labeled

## Changes

<!-- Bullet the notable code/docs changes. -->

## Validation

<!-- Check what you actually ran; paste output or note what could not run.
Do not claim a build, signing, notarization, or CI result that did not happen. -->

- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml --all-targets`
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] Experimental leg (if touched): the above with `--features experimental`

## Risks / follow-up

<!-- Known gaps, things not validated, and any follow-up work. -->
