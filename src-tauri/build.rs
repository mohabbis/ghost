use std::env;

fn main() {
    embed_comctl32_v6_manifest();
    tauri_build::build()
}

/// Embed the ComCtl32 v6 (`Microsoft.Windows.Common-Controls` 6.0.0.0) manifest
/// dependency when linking on Windows.
///
/// Tauri only links this manifest into the shipped application binary, not into
/// cargo's unit-test harness binary. Without it the test executable binds to
/// the legacy System32 ComCtl32 v5.82, which lacks exports that Tauri's Windows
/// dependency chain statically imports, so the loader aborts the process with
/// `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139) before any test runs. That is the
/// crash the Windows CI `Test` leg was hitting.
///
/// `cargo:rustc-link-arg` is the directive that reaches the `--lib` unit-test
/// harness (the `-tests` variant only covers `[[test]]` integration targets,
/// which this crate has none of). It also applies to the shipped binary, but
/// that already declares the same v6 dependency via Tauri's manifest, so the
/// duplicate is merged harmlessly by the linker.
///
/// See tauri-apps/tauri#13419 and tauri-apps/tauri#14580.
fn embed_comctl32_v6_manifest() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}
