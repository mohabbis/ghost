# 🌳 GitHub Issue Tree: Tech Stack Upgrade

*Context: Upgrading Ghost's tech stack while preserving the core trust model (`Approve → Execute → Audit → Undo`) and meeting YC submission timeline.*

---

## 📊 Priority Legend

| Priority | Label | Timeline | Description |
|----------|-------|----------|-------------|
| 🔴 P0 | `priority-0`, `yc-critical` | Week 1 | Required for YC submission or core functionality |
| 🟠 P1 | `priority-1`, `tech-upgrade` | Weeks 2-4 | Important improvements for tech stack upgrade |
| 🟡 P2 | `priority-2` | Weeks 4-6 | Important but can be deferred if timeline pressures arise |
| ⚪ P3 | `priority-3`, `post-yc` | Post-YC | Nice-to-have improvements after YC submission |

---

## 🎯 Epic 0: Upgrade Strategy & Guardrails *(P0 - Week 1)*

### #001 [EPIC] Tech Stack Upgrade: Strategy & Non-Goals
**Priority:** 🔴 P0  
**Labels:** `epic`, `priority-0`, `yc-critical`  
**Parent:** None  

#### Child Issues:
- [ ] #002 [TASK] Document upgrade goals: what we're improving vs. what stays sacred
- [ ] #003 [TASK] Define "trust boundary" preservation tests (no silent mutations, approval gating)
- [ ] #004 [TASK] Create migration checklist: data compatibility, user upgrade path, rollback plan
- [ ] #005 [TASK] Benchmark current perf baselines (startup time, scan speed, memory) for comparison
- [ ] #006 [TASK] Add "Upgrade Mode" feature flag to safely test new stack alongside old

**Why first?** Upgrading without guardrails risks breaking the product's core differentiator: trustworthy execution.

---

## 🧱 Epic 1: Backend Modernization *(P1 - Weeks 2-4)*

### #010 [EPIC] Rust Backend: Stability + Performance
**Priority:** 🟠 P1  
**Labels:** `epic`, `priority-1`, `tech-upgrade`, `rust`, `backend`  

#### 1A: Core Upgrades
- [ ] #011 [TASK] Upgrade Tauri to latest stable 2.x; audit breaking changes in plugin API
- [ ] #012 [TASK] Replace `reqwest 0.11` → `0.12` (hyper 1.x) for better async perf
- [ ] #013 [TASK] Evaluate `tokio 1.40+` runtime tuning for I/O-heavy Organizer scans
- [ ] #014 [TASK] Add `tracing-opentelemetry` for structured observability (opt-in, local-only)
- [ ] #015 [TASK] Profile `redb` vs. `sled` vs. `rocksdb` for workflow storage perf at scale
- [ ] #016 [TASK] Refactor `action_plan/` runtime to use `async-trait` + better error propagation
- [ ] #017 [TASK] Add `cargo-deny` + `cargo-audit` to CI; fail build on critical advisories

### #020 [EPIC] Storage: Migration Path + Future-Proofing
**Priority:** 🟠 P1  
**Labels:** `epic`, `priority-1`, `storage`, `data`  

#### 1B: Storage & Data Layer
- [ ] #021 [TASK] Formalize `redb` schema versioning with automatic migration tests
- [ ] #022 [TASK] Add encryption-at-rest toggle (AES-GCM) for audit logs (user-controlled key)
- [ ] #023 [TASK] Design "cloud sync ready" abstraction: local-first, sync-optional interface
- [ ] #024 [TASK] Deprecate `rusqlite` legacy import path after migration window closes
- [ ] #025 [TASK] Add `sqlite` feature flag for users who prefer SQLite backend (advanced)

---

## 🎨 Epic 2: Frontend Modernization *(P1 - Weeks 3-5)*

### #030 [EPIC] Frontend: Maintainable, Typed, Testable UI
**Priority:** 🟠 P1  
**Labels:** `epic`, `priority-1`, `frontend`, `typescript`  

#### 2A: From Vanilla JS → Modern Framework
- [ ] #031 [TASK] Evaluate framework: Svelte (lightweight) vs. SolidJS (perf) vs. React (ecosystem)
- [ ] #032 [TASK] Add TypeScript + strict mode; migrate `src/main.js` incrementally
- [ ] #033 [TASK] Componentize core views: Organizer, Plan Review, Guard Desk, Audit Log
- [ ] #034 [TASK] Add `vitest` + `@testing-library` for UI unit/integration tests
- [ ] #035 [TASK] Implement design system: tokens, components, dark mode support
- [ ] #036 [TASK] Add `i18n` scaffolding (Even if English-only now) for future localization
- [ ] #037 [TASK] Preserve "no bundler" dev experience or adopt `vite` with fast HMR

### #040 [EPIC] UX: Reduce Cognitive Load, Highlight Trust
**Priority:** 🟠 P1  
**Labels:** `epic`, `priority-1`, `ux`, `design`  

#### 2B: UX Clarity + Progressive Disclosure
- [ ] #041 [TASK] Redesign onboarding: "What Ghost does" in <60 seconds with interactive demo
- [ ] #042 [TASK] Add "Mode Selector" homepage: Organizer (default) / Record / Guard / Advanced
- [ ] #043 [TASK] Visualize trust pipeline: animated `Record→Inspect→Approve→Execute→Undo` flow
- [ ] #044 [TASK] Add "Why this matters" tooltips for technical terms (semantic AX, WAL, etc.)
- [ ] #045 [TASK] Surface "Feature Status" badge: ✅ Production / 🟡 Beta / 🚫 Disabled
- [ ] #046 [TASK] A/B test simplified vs. advanced UI for new user retention

---

## 🍎 Epic 3: Native Platform Enhancements *(P2 - Weeks 4-6)*

### #050 [EPIC] macOS: Accessibility + SwiftUI Polish
**Priority:** 🟡 P2  
**Labels:** `epic`, `priority-2`, `macos`, `native`  

#### 3A: macOS Deep Integration
- [ ] #051 [TASK] Upgrade `GhostAXHelper.swift` to support `AXUIElement` batch queries
- [ ] #052 [TASK] Add native macOS menu bar app with quick Organizer trigger (⌘+Shift+G)
- [ ] #053 [TASK] Integrate with macOS Shortcuts app for workflow launching
- [ ] #054 [TASK] Add VoiceOver/NVDA accessibility testing suite + ARIA audit
- [ ] #055 [TASK] Sign macOS builds with Apple Notarization + hardened runtime by default
- [ ] #056 [TASK] Add Apple Silicon + Intel universal binary CI validation

### #060 [EPIC] Windows: Trust + Reliability
**Priority:** 🟡 P2  
**Labels:** `epic`, `priority-2`, `windows`, `native`  

#### 3B: Windows Parity + Signing
- [ ] #061 [TASK] Implement Azure Trusted Signing for Windows installer (remove SmartScreen warning)
- [ ] #062 [TASK] Add Windows Accessibility (UI Automation) backend parity with macOS AX
- [ ] #063 [TASK] Test Ghost on Windows ARM64 (Surface Pro X, new Copilot+ PCs)
- [ ] #064 [TASK] Add Windows-specific UX: File Explorer context menu integration
- [ ] #065 [TASK] Document Windows Defender exclusion guidance for Organizer scans

---

## 🔐 Epic 4: Security & Compliance Hardening *(P1 - Ongoing)*

### #070 [EPIC] Security: Audit, Harden, Document
**Priority:** 🟠 P1  
**Labels:** `epic`, `priority-1`, `security`  

- [ ] #071 [TASK] Run `cargo audit` + `cargo deny` in CI; publish results publicly
- [ ] #072 [TASK] Replace `'unsafe-inline'` CSP with nonce-based approach for production
- [ ] #073 [TASK] Add integrity checks for downloaded workflows/plugins (SHA-256 + signature)
- [ ] #074 [TASK] Document threat model: what Ghost protects against, what it doesn't
- [ ] #075 [TASK] Add "Security Status" page in-app: signing, updates, permissions, audit log
- [ ] #076 [TASK] Prepare for SOC 2 Type I readiness (if targeting enterprise later)
- [ ] #077 [TASK] Add automated pen-test workflow (OWASP ZAP) against local MCP server

---

## 🚀 Epic 5: Build, CI/CD, & Distribution *(P1 - Weeks 1-3)*

### #080 [EPIC] Release Engineering: Fast, Safe, Transparent
**Priority:** 🟠 P1  
**Labels:** `epic`, `priority-1`, `ci-cd`, `release`  

- [ ] #081 [TASK] Sync GitHub Releases version with marketing site (fix v2.0.3 gap)
- [ ] #082 [TASK] Add "Technical Preview" badge to pre-release builds + in-app version display
- [ ] #083 [TASK] Automate SHA256 checksums + GPG signature verification instructions
- [ ] #084 [TASK] Add "Build from Source" guide with one-liner for devs (`make dev`)
- [ ] #085 [TASK] Implement staged rollouts: 10% → 50% → 100% for auto-updates
- [ ] #086 [TASK] Add crash reporting (opt-in, local-first, anonymized) via `backtrace` + `minidump`
- [ ] #087 [TASK] Create "Release Health" dashboard: install success rate, crash-free sessions

---

## 🧪 Epic 6: Experimental Features (Gated) *(P3 - Post-YC)*

### #090 [EPIC] Experimental: Local AI + MCP (Behind Feature Flag)
**Priority:** ⚪ P3  
**Labels:** `epic`, `priority-3`, `experimental`, `ai`, `post-yc`  

- [ ] #091 [TASK] Isolate `candle` LLM inference behind `--features experimental`
- [ ] #092 [TASK] Add "Suggestion Mode": AI proposes plans, user approves, deterministic code executes
- [ ] #093 [TASK] Build local MCP server with signed approval tokens (no cloud dependency)
- [ ] #094 [TASK] Add "Observer Mode" prototype: read-only AX scanning + plan preview (no execution)
- [ ] #095 [TASK] Document experimental feature risks + disable-by-default policy
- [ ] #096 [TASK] Add "Feature Feedback" in-app: let users vote on what to stabilize next

---

## 🗓️ Prioritization for YC Submission (Next 4 Weeks)

| Priority | Issue | Why | Effort |
|----------|-------|-----|--------|
| 🔴 P0 | #001-#006 | Define upgrade guardrails | 2 days |
| 🔴 P0 | #081-#083 | Fix release/version confusion | 1 day |
| 🟠 P1 | #031-#033 | Start frontend TS migration | 3-5 days |
| 🟠 P1 | #011-#012 | Tauri/reqwest upgrades | 2 days |
| 🟠 P1 | #041-#043 | UX clarity improvements | 3 days |
| 🟡 P2 | #061 | Windows signing (critical for trust) | 2-3 days |
| 🟡 P2 | #071-#073 | Security hardening | 2 days |
| ⚪ P3 | #090+ | Experimental features | Post-YC |

---

## 🔄 Migration Strategy: Zero-Downtime Upgrade

```
Current v2.0.3
    ↓
Add Feature Flag: new_stack
    ↓
User enables flag?
    ├─ No → Run existing vanilla JS + Rust
    └─ Yes → Run new TS frontend + upgraded Rust
              ↓
        Write to same redb storage
              ↓
        Seamless data compatibility
              ↓
        User can toggle back anytime
              ↓
        Gradual rollout after testing
```

**Key principle**: The upgrade should be *invisible* to users who don't opt in. Data stays compatible. Rollback is one toggle.

---

## 📦 Tech Stack Upgrade Decision Matrix

| Component | Current | Upgrade Candidate | Keep? | Why |
|-----------|---------|------------------|-------|-----|
| Frontend | Vanilla JS | Svelte + TypeScript | ✅ Yes | Better DX, type safety, small bundle |
| Backend | Rust + Tauri 2 | Rust + Tauri 2.1+ | ✅ Yes | Stay in trusted ecosystem |
| Storage | redb | redb + encryption toggle | ✅ Yes | Pure Rust, no C deps, fast |
| Build | Cargo + Make | Cargo + Justfile + GitHub Actions | ✅ Yes | Simpler, more maintainable |
| Native macOS | SwiftUI + AX | SwiftUI + AX + Shortcuts | ✅ Yes | Deepen macOS integration |
| Native Windows | enigo + UIA | enigo + Azure Signing | ✅ Yes | Fix trust gap on Windows |
| AI/LLM | candle (optional) | candle + better gating | ✅ Yes | Keep local-first, experimental |
| Cloud Sync | Disabled | Abstract interface, local-first | ✅ Yes | Future-proof without committing |

---

## 🦜 Guiding Principles

> **"Upgrade the engine, not the promise."**

Ghost's differentiator isn't the tech stack — it's the **trust pipeline**. Any upgrade must:

1. ✅ Preserve `Approve → Execute → Audit → Undo` as non-negotiable
2. 🔒 Keep data local-first, encrypted, user-controlled
3. 🎯 Make the simple path obvious, the powerful path discoverable
4. 🚀 Ship YC-ready improvements first (clarity, trust, reliability)

**Next step**: Pick one P0 issue (#001 or #081) and ship it this week. Momentum > perfection.

---

## 📝 How to Use This Issue Tree

1. **Create issues using templates**: Navigate to `/issues/new/choose` in GitHub and select the appropriate template
2. **Link child issues**: Update child issues to reference their parent epic
3. **Track progress**: Use GitHub Projects or a project board to visualize progress across epics
4. **Prioritize ruthlessly**: Focus on P0 and P1 items for YC submission; defer P3 to post-YC

### Quick Links to Templates:
- 🎯 [Epic Template](../.github/ISSUE_TEMPLATE/epic.md)
- 🔴 [P0 Critical Task](../.github/ISSUE_TEMPLATE/task-p0.md)
- 🟠 [P1 High Priority Task](../.github/ISSUE_TEMPLATE/task-p1.md)
- 🟡 [P2 Medium Priority Task](../.github/ISSUE_TEMPLATE/task-p2.md)
- ⚪ [P3 Low Priority Task](../.github/ISSUE_TEMPLATE/task-p3.md)
- 🛡️ [Security Task](../.github/ISSUE_TEMPLATE/security.md)
- 🧪 [Experimental Feature](../.github/ISSUE_TEMPLATE/experimental.md)
- 📚 [Documentation Task](../.github/ISSUE_TEMPLATE/docs.md)
- 🐞 [Bug Report](../.github/ISSUE_TEMPLATE/bug.md)
