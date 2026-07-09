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
3. **Windows CI Test DLL Crash Resolved:**
   * Changed `crate-type` of `ghost_lib` from `["staticlib", "cdylib", "rlib"]` to strictly `["rlib"]` in `src-tauri/Cargo.toml`.
   * This prevents Cargo tests on Windows from looking for/loading a dynamically compiled `ghost_lib.dll` which lacks test symbols and causing `0xc0000139` crashes.
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
