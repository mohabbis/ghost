# Ghost — The Trust Layer for AI Agents

[![Download](https://img.shields.io/badge/Download-Latest-8d7bff?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![macOS](https://img.shields.io/badge/macOS-12+-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Every company will run hundreds of AI agents by 2027. None can be trusted with sensitive operations.**

Ghost is the human-in-the-loop approval, audit, and undo layer that makes autonomous AI agents safe for enterprise use.

## The Problem

- AI agents (Claude, Cursor, Windsurf) can access your files, databases, and APIs
- One hallucination could delete production data or leak PII
- Current solutions: manual review (slow) or blind trust (risky)

## The Ghost Solution

Ghost sits between AI agents and your sensitive operations via the **MCP protocol**:

```
AI Agent → MCP Request → Ghost Policy Check → Human Approval → Execute → Audit Log → Undo if needed
```

**What Ghost does:**
- **Intercepts** every action via MCP protocol (Claude Desktop, Cursor, etc.)
- **Policies** enforce rules: "no deletions without approval", "redact PII", "max $100 transactions"
- **Approvals** require human sign-off for high-risk actions
- **Audit** logs every decision for SOC2/HIPAA/GDPR compliance
- **Undo** rolls back any mistake within seconds

## Live Demo: Claude + Ghost

```bash
# 1. Connect Ghost to Claude Desktop
# Add to ~/Library/Application Support/Claude/claude_desktop_config.json:
{
  "mcpServers": {
    "ghost": {
      "command": "ghost",
      "args": ["mcp", "serve"]
    }
  }
}

# 2. Ask Claude: "Process all unpaid invoices in ~/Downloads"
# Ghost will:
#   - Intercept the file access request
#   - Check policy: "Require approval for batch operations"
#   - Show preview in Ghost UI
#   - You approve/reject each action
#   - Audit log created automatically
#   - Undo available for 30 days
```

## Architecture

- **MCP Server**: Standard protocol connecting to Claude, Cursor, Windsurf, etc.
- **Policy Engine**: Capability-based access control with PII redaction
- **Deterministic Compression**: Converts raw events into semantic workflows
- **Vault Encryption**: AES-256 encryption for sensitive audit logs
- **Native macOS**: Swift + Rust hybrid for performance + security
- **35K+ lines** of production Rust code

## Who Uses Ghost?

| User | Use Case | Value |
|------|----------|-------|
| **AI Early Adopters** | Engineers using Claude/Cursor daily | Safety guards against hallucinations |
| **Bookkeepers** | Process invoices/payments with AI | Retain control, automate safely |
| **Compliance Teams** | SOC2, HIPAA, GDPR audit trails | Automated compliance logging |
| **SMB Operations** | Replace $50k/year enterprise tools | $29/month alternative |

## What Ghost Is NOT

- ❌ Not another AI chatbot
- ❌ Not a general-purpose file organizer
- ❌ Not an enterprise RPA platform (yet)
- ❌ Not a replacement for human judgment

## Quick Start

### Download
- [macOS Universal Binary](https://github.com/mohabbis/ghost/releases/latest)
- Requires macOS 12+ (Monterey or later)

**⚠️ Gatekeeper Warning**: If you see "App can't be opened", run:
```bash
xattr -dr com.apple.quarantine /Applications/Ghost.app
```

### Build from Source
```bash
# Clone
git clone https://github.com/mohabbis/ghost.git
cd ghost

# Build native macOS app
cd apps/macos
swift build --configuration Release

# Or build Tauri version
npm install
npm run tauri build
```

## Roadmap

- **Q1 2025**: MCP integrations (Claude, Cursor, Windsurf), policy templates
- **Q2 2025**: Team collaboration, shared approval workflows
- **Q3 2025**: Windows/Linux support, marketplace for custom policies
- **Q4 2025**: AI training mode (learn from approvals to auto-approve safe patterns)

## Metrics That Matter

- 🎯 **Weekly Active Workflows**: Teams running 10+ approved AI operations/week
- ⚡ **Time Saved**: 15 minutes per approved workflow vs manual review
- 🛡️ **Blocked Risks**: High-risk actions caught by policies before execution
- ↩️ **Undo Rate**: <5% (proves approvals work, but safety net exists)

## Join the Revolution

We're building the trust layer for the autonomous future.

- 📧 **Early Access**: founders@ghost.dev
- 💬 **Discord**: [Join community](https://discord.gg/ghost)
- 🐦 **Twitter**: [@ghost_dev](https://twitter.com/ghost_dev)
- 📋 **YC Application**: Applying W25 batch

---

*"In 5 years, no enterprise will run AI agents without a Ghost-like approval layer."*
