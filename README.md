# Ghost

[![Download](https://img.shields.io/badge/Download-Latest-8d7bff?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![Release](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/release.yml?style=flat-square&label=Release)](https://github.com/mohabbis/ghost/actions/workflows/release.yml)
[![macOS](https://img.shields.io/badge/macOS-12%2B-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10%2F11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Ghost is the deterministic trust layer for desktop automation.**

<p align="center">
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 220" width="100%" height="auto" style="background:#070813; border-radius:12px; border:1px solid #1f2033; font-family:system-ui, -apple-system, sans-serif;">
    <defs>
      <linearGradient id="violet-glow" x1="0%" y1="0%" x2="100%" y2="0%">
        <stop offset="0%" stop-color="#8d7bff" stop-opacity="0.2"/>
        <stop offset="50%" stop-color="#ab9dff" stop-opacity="1"/>
        <stop offset="100%" stop-color="#8d7bff" stop-opacity="0.2"/>
      </linearGradient>
      <filter id="glow" x="-20%" y="-20%" width="140%" height="140%">
        <feGaussianBlur stdDeviation="6" result="blur" />
        <feComposite in="SourceGraphic" in2="blur" operator="over" />
      </filter>
    </defs>
    <style>
      @keyframes pulse-glow {
        0%, 100% { opacity: 0.3; }
        50% { opacity: 0.9; }
      }
      @keyframes flow-line {
        to { stroke-dashoffset: -40; }
      }
      .flow-path {
        stroke: #8d7bff;
        stroke-width: 2.5;
        stroke-dasharray: 8 4;
        animation: flow-line 2s linear infinite;
      }
      .flow-bg {
        stroke: #1f2033;
        stroke-width: 2.5;
      }
      .node-border {
        stroke: #1f2033;
        stroke-width: 1.5;
        fill: #0c0d21;
      }
      .node-border.active-violet {
        stroke: #8d7bff;
        fill: #11132a;
      }
      .node-border.active-warn {
        stroke: #ffb86b;
        fill: #1a1622;
      }
      .node-border.active-success {
        stroke: #83f6c4;
        fill: #0d1e21;
      }
      .pulse-dot {
        fill: #8d7bff;
        animation: pulse-glow 2s infinite ease-in-out;
      }
      .step-text {
        font-size: 11px;
        font-weight: 600;
        fill: #b9b3cc;
        text-anchor: middle;
      }
      .step-title {
        font-size: 13px;
        font-weight: bold;
        fill: #f7f4ff;
        text-anchor: middle;
      }
    </style>
    <!-- Connective flow lines -->
    <line x1="130" y1="110" x2="310" y2="110" class="flow-bg" />
    <line x1="310" y1="110" x2="490" y2="110" class="flow-bg" />
    <line x1="490" y1="110" x2="670" y2="110" class="flow-bg" />
    <line x1="130" y1="110" x2="310" y2="110" class="flow-path" />
    <line x1="310" y1="110" x2="490" y2="110" class="flow-path" style="animation-delay: -0.66s; stroke:#ffb86b;" />
    <line x1="490" y1="110" x2="670" y2="110" class="flow-path" style="animation-delay: -1.33s; stroke:#83f6c4;" />
    <!-- Node 1: Record / Scan -->
    <g transform="translate(130, 110)">
      <circle r="42" class="node-border active-violet" style="filter: drop-shadow(0 0 10px rgba(141,123,255,0.15));" />
      <circle r="4" cy="-18" class="pulse-dot" />
      <text y="5" class="step-title">1. RECORD</text>
      <text y="20" class="step-text">Scan &amp; Parse</text>
    </g>
    <!-- Node 2: Guard Policy Check -->
    <g transform="translate(310, 110)">
      <circle r="42" class="node-border active-warn" style="filter: drop-shadow(0 0 10px rgba(255,184,107,0.15));" />
      <circle r="4" cy="-18" class="pulse-dot" style="fill:#ffb86b;" />
      <text y="5" class="step-title" style="fill:#ffb86b;">2. GUARD</text>
      <text y="20" class="step-text">Policy Checks</text>
    </g>
    <!-- Node 3: Human Approval -->
    <g transform="translate(490, 110)">
      <circle r="42" class="node-border active-success" style="filter: drop-shadow(0 0 10px rgba(131,246,196,0.15));" />
      <circle r="4" cy="-18" class="pulse-dot" style="fill:#83f6c4;" />
      <text y="5" class="step-title" style="fill:#83f6c4;">3. APPROVE</text>
      <text y="20" class="step-text">Dry-Run Preview</text>
    </g>
    <!-- Node 4: Replay / Execute -->
    <g transform="translate(670, 110)">
      <circle r="42" class="node-border active-violet" style="filter: drop-shadow(0 0 15px rgba(141,123,255,0.2));" />
      <circle r="4" cy="-18" class="pulse-dot" />
      <text y="5" class="step-title">4. REPLAY</text>
      <text y="20" class="step-text">Audit &amp; Undo</text>
    </g>
  </svg>
</p>

Ghost lets users automate sensitive desktop file workflows by recording once, reviewing exactly what happens, approving within policy boundaries, auditing every operation, and undoing changes when needed.

## The Problem

Bookkeepers, practice admins, and operations teams spend hours every week organizing invoices, receipts, statements, and exports across local folders and desktop apps. This work is repetitive, high-frequency, and painful — but it is also too sensitive for black-box AI agents, too desktop-specific for Zapier or Make, and too small-business-oriented for enterprise RPA.

Existing tools don't fit:

- **Macro recorders** replay blind coordinates; they break when windows move.
- **Zapier/Make** excel at cloud APIs, not local file workflows.
- **UiPath/Enterprise RPA** are powerful but expensive and overbuilt for SMBs.
- **AI agents** are unpredictable and hard to audit—terrifying with financial files.
- **Manual scripts** require technical skill.
- **Keyboard Maestro/BetterTouchTool** don't offer audit trails, policy enforcement, or undo.

## Ghost's Answer

Ghost is not an autonomous agent. Ghost is a **deterministic, auditable automation engine** built around a simple pipeline:

```
Intent → Plan → Policy Check → Human Approval → Execution → Audit → Undo
```

**Deterministic**: Same input always produces the same output. No hallucination. No surprise behavior.

**Auditable**: Every operation is logged with full context. Workflows are reviewable before execution.

**Policy-enforced**: Users define Zones (folders Ghost may touch) and Capabilities (what Ghost may do). Everything else is denied by default.

**Undoable**: Reversible operations write undo data before execution. Users can recover from mistakes.

## Get Ghost

| Platform | Download |
|----------|----------|
| macOS (Apple Silicon + Intel) | [Ghost.dmg](https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg) |
| Windows 10 / 11 (64-bit) | [Ghost_Setup.exe](https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe) |

> **Note:** current builds are developer-preview quality. macOS builds may be ad-hoc signed; if macOS blocks the app, approve it under **System Settings → Privacy & Security**. Notarized releases are in progress.

## How It Works: Ghost Organizer

The starting workflow is **Ghost Organizer**: safe, reviewable file organization with audit and undo.

### Step-by-Step Flow

1. **Select Zone** — Choose a folder Ghost may access (e.g., Downloads, Desktop).
2. **Scan Files** — Ghost analyzes filenames, extensions, and patterns.
3. **Classify** — Deterministic rules categorize files (invoices, receipts, statements, etc.).
4. **Propose Plan** — Ghost suggests moves and renames, with confidence scores and matched rules.
5. **Review & Approve** — User sees before/after preview. Can edit destinations, deselect actions, or cancel.
6. **Policy Check** — Ghost Guard flags conflicts, overwrites, and out-of-Zone operations.
7. **Write Undo Journal** — Before touching anything, Ghost writes a local undo record.
8. **Execute** — Only approved actions run. Progress is shown step-by-step.
9. **Audit** — Every operation logged with timestamps, decisions, and hashes.
10. **Undo** — One-click recovery of the entire run.

### Demo: Five Minutes

1. Open Ghost Organizer.
2. Select `~/Downloads`.
3. Click "Scan."
4. Ghost proposes: "Move `invoice_acme.pdf` to `/Clients/Acme/Invoices/2026/`" with matched rule shown.
5. Review the before/after tree. Click "Approve."
6. Ghost executes and audits. Shows: "43 moved, 31 renamed, 5 folders created, 7 skipped, 0 blocked."
7. Undo is available.

That's the product. Boring. Trustworthy.

## Who Ghost Is For

- **Bookkeepers and practice admins** organizing client invoices, receipts, statements, and exports.
- **Office managers** and small business operations teams drowning in downloads and attachments.
- **Students and nonprofits** managing financial or operational workflows that require audit trails.

Ghost is deliberately **not** for:

- Autonomous agents you launch and forget about.
- Unattended, scheduled automation.
- Workflows that don't need review before execution.

Every meaningful action requires your approval.

## The Trust Pipeline

Ghost's architecture enforces one principle:

> **Ghost may suggest anything, but it only does what you have explicitly approved, inside boundaries you control.**

Every operation flows through the same pipeline — no exceptions:

```
Intent → Plan → Policy Check → Human Approval → Execution → Audit Log → Undo Path
```

### Core Guarantees

- **Deny by default** — Operations are blocked unless explicitly allowed by your Zones and Capabilities.
- **Deterministic execution** — Suggestions (including any AI-assisted ones) never execute directly; only reviewed, approved plans do.
- **Suppressed secrets** — Secure-field input is never retained—not in logs, not in recordings, not in previews.
- **Local-first** — Workflows, logs, and settings live on your machine. No account. No cloud dependency. No telemetry unless you opt in.
- **Auditable** — Every run leaves a tamper-evident audit log and undo journal.

## Current Status

**Early-stage but functional.** Working now:

- ✅ **Ghost Organizer** — Safe file organization with preview, policy checks, audit, and undo
- ✅ **Deterministic compression** — Semantic steps from raw events
- ✅ **Ghost Guard policy engine** — Deny-by-default rules engine
- ✅ **Zones and Capabilities** — Local boundary enforcement
- ✅ **Audit logs and undo journals** — Full operation history with recovery
- ✅ **macOS and Windows** — Desktop builds for both platforms

**Not yet production-ready:**

- ⚠️ **Release signing** — macOS notarization and Windows code signing in progress
- ⚠️ **Cross-app reliability** — Some workflows still depend on screen coordinates (fallback for robustness)
- ⚠️ **Richer target resolution** — Window-title awareness and window-relative positioning in development

## The Wedge: Ghost Organizer

Why start with file organization?

1. **High frequency** — Bookkeepers and admins do this every week.
2. **Sensitive** — Files contain financial and client data. Black-box automation is unacceptable.
3. **Audit-heavy** — These users need compliance trails anyway.
4. **Measurable ROI** — 2–3 hours/week saved per user is easy to quantify.
5. **Clear UX** — Folder preview and file list are familiar patterns.

Ghost Organizer proves the trust pipeline works for a real, painful workflow. Then we expand.

## Roadmap

### Phase 1: Scope Reduction (Weeks 1–2)
- Remove AI language and experimental features from public UI
- Publish formal trust pipeline spec
- Define canonical workflows and success metrics

### Phase 2: Reliability Foundation (Weeks 3–5)
- Build benchmark suite with ≥99.5% success target for canonical workflows
- Add comprehensive unit and integration tests
- Publish reliability metrics

### Phase 3: Production Hardening (Weeks 6–8)
- Apple Developer ID signing and notarization
- Windows code signing
- Security documentation and threat model

### Phase 4: Organizer UX Polish (Weeks 9–10)
- First-run onboarding flow
- Improved preview and policy UI
- Dark mode and keyboard shortcuts

### Phase 5–8: Traction and YC (Months 4–6)
- Private beta with 10 users
- 3 paid pilots from SMB firms
- Public beta launch
- YC application with metrics and testimonials

## Architecture

**Stack**: Rust backend (Tauri 2), vanilla HTML/CSS/JS frontend, local SQLite storage.

**Core Modules**:
- `organizer/` — File scanner, classifier, planner, executor, undo
- `policy/` — Deny-by-default capability engine
- `audit/` — Append-only audit logs and undo journals
- `core/compression/` — Event compression to semantic steps
- `platform/` — macOS and Windows capture/replay backends

See [`IMPLEMENTATION.md`](IMPLEMENTATION.md) for detailed status and [`docs/`](docs/) for architecture specs.

## Security & Privacy

- **Local-first by default** — No cloud, no account, no telemetry unless opted in.
- **No camera, no microphone** — Input capture only during explicit recording/approved replay.
- **Suppress secrets** — Secure fields are never stored, logged, or transmitted.
- **Signed releases** — Code signing and notarization for safe installation.

See [`SECURITY.md`](SECURITY.md) for threat model and responsible disclosure.

## Development

```bash
# Install Rust and Tauri CLI
cargo install tauri-cli --version "^2.0" --locked

# Run checks
make ci         # fmt + test + clippy
make check      # cargo check
make test       # cargo test
make dev        # cargo tauri dev

# Build
make build      # cargo tauri build --no-bundle
```

See [`AGENTS.md`](AGENTS.md) for contribution guidelines, architecture details, and the binding contract for humans and AI agents working on Ghost.

## Contributing

Contributions are focused on:
- Replay reliability and semantic target resolution
- Ghost Organizer UX and correctness
- Safety and policy hardening
- Docs and examples
- Bug fixes

Not accepting:
- Feature bloat or experimental half-finished work
- Cloud-dependent features (local-first is non-negotiable)
- Autonomous execution modes (user approval is the product)

## FAQ

**Does my data leave my machine?**
No. Workflows, logs, and settings are local. No account. No server. Telemetry is off by default.

**Can Ghost delete files?**
Not in Ghost Organizer—delete is blocked by default. All mutations are previewed, approved, audited, and undoable.

**What about passwords?**
Secure-field input is suppressed at capture time and never retained. Ghost Guard blocks replay of workflows containing secrets.

**Can it run while I'm away?**
No. Replay runs when you start it and stays interruptible. Unattended execution is deliberately not a current feature.

**Why not just use a macro recorder?**
Raw macros replay coordinates and break when windows move—and they'll happily type your password into the wrong field. Ghost reviews, guards, re-resolves targets semantically, and leaves an audit trail.

## Kubernetes Deployment

To support scaling out marketing landing nodes (Y Combinator scale preparation), Ghost includes containerization and Kubernetes orchestration configs:

* **Dockerfile:** Uses a lightweight Nginx base to package and serve the static marketing site.
* **k8s-deployment.yaml:** Spawns a 3-replica LoadBalancer-backed service of marketing nodes with strict CPU/Memory resource bounds and liveness/readiness check probes.

```bash
# Build the Docker container
docker build -t mohabbis/ghost-marketing:latest .

# Deploy to a Kubernetes cluster (Minikube or GKE)
kubectl apply -f k8s-deployment.yaml
```

## License

MIT — see [LICENSE](LICENSE).

---

**Ghost is boring. That is the point.** Trust means predictability, auditability, and control. Ghost is built to earn your trust by showing exactly what it does—before, during, and after.
