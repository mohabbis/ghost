# Ghost Project — Detailed Handoff Prompt for Continued Development

**Last Updated:** 2026-07-09  
**Status:** Clean, Compiling, PR 129 & PR 130 Merged into Master  

---

## 🎯 What You're Picking Up
You are inheriting **Ghost**, a Tauri-based local-first desktop automation tool. Recently, the app was prepared for Muhammad's internship at **PLS Financial Services** by adding an interactive **AI Copilot** workspace and generalizing the financial cashing automation demo.

Key Achievements:
1. **Security Vulnerabilities Fixed (PR 129):** Resolved critical warnings in `crossbeam-epoch`, `quick-xml` (via `plist`), and `anyhow`. The Tauri backend compiles cleanly (`cargo check` passes).
2. **AI Copilot Workspace (PR 130):** Implemented a general-purpose compliance check & macro replay desk in the Tauri app (`src/index.html`, `src/main.js`).
3. **Cohesive UI/UX Styling:** Refactored the UI from looking like a legacy outdated bank screen to a gorgeous, premium **glassmorphic dark-mode** design matching the app's aesthetic.
4. **General-Purpose Niche:** Updated the landing page (`public/index.html`) use cases to feature **Financial Operations**, referencing PLS as an integration target.

---

## 📁 Recent Changes (This Session)
* **`src/index.html`:** Added the `✨ AI Copilot` tab and built a modern dark glassmorphic document scanner/POS data entry form simulator.
* **`src/main.js`:** Integrated the `plsInit` event controller to handle scan animations, scenarios (Payroll Valid, Out-of-State Limit Warn, Signature Mismatch Fail), and programmatic field autofill replay logs.
* **`public/index.html`:** Renamed and generalized the PLS niche section to a broad "Financial Operations" case study.
* **`src-tauri/Cargo.lock`:** Locked dependency versions to resolve security vulnerabilities.

---

## 🚀 Immediate Next Steps (Internship Start)
When Muhammad starts his internship at PLS and identifies specific department workflows, proceed with the following:

### 1. Map Real Legacy POS Input Fields
* Audit the HTML/window elements of the actual legacy point-of-sale software in the PLS branch offices.
* Replace the simulation selectors inside `src/main.js` with real input descriptors.

### 2. Integrate Real OCR & KYC Scanning
* Hook up the scanner action button to a local OCR engine (such as Tesseract or a lightweight local Python sidecar service) to extract check text and ID attributes locally without sending data to the cloud.

### 3. Build Hardcoded Compliance Rule Templates
* Create JSON templates (`limit_presets.json`) to allow branch managers to configure store cashing limits, out-of-state checks, and signature verification thresholds.

---

## 📚 Key Files Modified
* **`src/index.html`** — `✨ AI Copilot` tab structure and modern layout.
* **`src/main.js`** — Simulator controllers, scenarios, scan timelines, and typing simulators.
* **`public/index.html`** — Niche sections for financial teller marketing copy.
* **`src-tauri/Cargo.lock`** — Fixed cargo audit warnings.
