# Ghost Project — Detailed Handoff Prompt for Continued Development

**Last Updated:** 2026-07-09  
**Status:** Clean, Compiling locally, Master CI build resolved, PRs 129 & 130 fully merged.

---

## 🎯 What You're Picking Up
You are inheriting **Ghost**, a Tauri-based local-first desktop automation tool. Recently, the app was prepared for Muhammad's internship at **PLS Financial Services** and structured to align with a Y Combinator submission:
* **The Trust Pipeline (Deterministic execution):** Treat AI suggestions as proposals; deterministic code executes only what the user reviews, approves, and can undo.
* **Local-First Privacy:** Runs entirely on-device with local database and local capabilities, making it secure and SOC2-ready out of the box.

---

## 📁 Recent Changes (This Session)
1. **AI Copilot view (`src/index.html` & `src/main.js`):**
   * Implemented a general-purpose **AI Copilot** tab and compliance scanner/macro replay simulator.
   * Scenarios (Payroll Valid, Out-of-State Warning, Signature Mismatch Failure) demonstrate scanning IDs/checks and replaying automated data entry step-by-step into a legacy terminal POS form.
   * Redesigned the UI cards for scanned checks and IDs using a high-end, **glassmorphic dark-mode** style.
2. **Vulnerability Patches (Cargo.lock):**
   * Updated `crossbeam-epoch`, `quick-xml`, and `anyhow` to resolve critical vulnerabilities.
3. **Windows CI Test Crash (`0xc0000139`) Resolved:**
   * Root cause: Tauri links the `Microsoft.Windows.Common-Controls` v6 manifest only into the shipped app binary, not into cargo's `--lib` unit-test harness. Without it the test exe falls back to the legacy ComCtl32 v5.82 and the loader aborts with `STATUS_ENTRYPOINT_NOT_FOUND` before any test runs. (The earlier `crate-type = ["rlib"]` change in `src-tauri/Cargo.toml` did not address this and is retained only because desktop builds do not need the mobile `staticlib`/`cdylib` outputs.)
   * Fix: `src-tauri/build.rs` emits a Windows-only `cargo:rustc-link-arg=/MANIFESTDEPENDENCY:...Common-Controls 6.0.0.0...` so the v6 manifest is embedded into the test binary too. The fragile PowerShell-based `is_cargo_test()` skip of `tauri_build::build()` was removed. See tauri-apps/tauri#13419 / #14580.
4. **Version Bump:**
   * Bumped package version from `1.1.0` to `1.2.0` in both `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
5. **Kubernetes & Containerization (Y Combinator Scale Prep):**
   * Created a `Dockerfile` to package the static marketing landing page (`public/`) using Nginx.
   * Created a `k8s-deployment.yaml` manifest that spawns a load-balanced, 3-replica cluster of marketing website nodes with resource limits and health checks.
   * Documented deployment commands in `README.md` alongside an animated SVG pipeline diagram.

---

## 🚀 Immediate Next Steps
1. **Tag & Trigger v1.2.0 Release:**
   * Push a git tag `v1.2.0` to the remote repository. This triggers the GitHub Actions `Release` workflow to build signed binaries (`Ghost.dmg` and `Ghost_Setup.exe`) and upload them to the GitHub release.
   * The Vercel-deployed marketing site will automatically serve these new downloads as they point to `releases/latest`.
2. **Integrate Real OCR selectors:**
   * Transition the AI Copilot from mock scanning animations to real local OCR selectors when the internship starts.
