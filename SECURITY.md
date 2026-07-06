# Security Policy

## Overview

Ghost is designed around a **local-first, deny-by-default trust model**. This document covers Ghost's security principles, threat model, responsible disclosure, and supported versions.

**Core principle**: Ghost may suggest anything, but it only executes what you have explicitly approved, inside boundaries you control.

## Security Principles

1. **Local-first by default** — Files and workflows never leave your machine without explicit user action.
2. **Deny-by-default** — All operations are blocked unless explicitly allowed by user-defined Zones and Capabilities.
3. **Deterministic execution** — Same input always produces the same output. No hidden behavior.
4. **Auditable** — Every operation is logged with full context. Users can review and export audit trails.
5. **Reversible** — Undo data is written before any destructive operation.
6. **No autonomous execution** — User approval is required before any meaningful operation runs.
7. **Secrets suppressed** — Secure-field input is never retained, even with audit enabled.
8. **Minimal permissions** — Ghost requests only the OS permissions it actually needs.

## Threat Model

### Assets

| Asset | Value | Protection |
|-------|-------|-----------|
| User files | High | Zones, Capabilities, approval gate, undo |
| Financial documents | High | Audit logs, policy engine, access control |
| Audit logs | High | Append-only, hash-chained, user-controlled export |
| Workflows | Medium | Local encrypted storage (optional password) |
| Session data | Low | Ephemeral, cleared on exit |
| Undo journals | Medium | Cryptographic hash validation |

### Threats

| Threat | Scenario | Mitigation | Residual Risk |
|--------|----------|-----------|---------------|
| **Accidental file loss** | User approves a plan without reading | Explicit approval + before/after preview + undo | User error (mitigated by UX) |
| **Silent overwrite** | File exists at destination | Conflict detection + approval gate | Low (requires user action) |
| **Out-of-Zone mutation** | Workflow targets file outside Zone | Policy engine blocks by default | Low (policy enforced) |
| **Path traversal** | Malicious workflow includes `../` paths | Path sanitization + Zone checks | Low (defense in depth) |
| **Symlink attack** | Symlink points outside Zone | Symlink resolution + Zone validation | Medium (OS-dependent) |
| **Compromised workflow file** | Attacker modifies workflow JSON | Workflow integrity validation + version pinning | Medium (mitigated by local storage + backups) |
| **Audit log tampering** | Attacker modifies audit entries | Hash chain validation (entry hash = hash of previous + current) | Low (tampering detectable) |
| **Undo journal failure** | Undo data lost or corrupted | Journal integrity checks + hash validation | Low (rare, caught in tests) |
| **Privilege escalation** | User runs Ghost with elevated privileges | Not recommended; warn in UI | User choice (mitigated by education) |
| **Unattended execution** | User leaves Ghost running with auto-execute | Not a current feature (user approval required) | Low (not implemented) |
| **Sensitive data in logs** | PII or secrets captured in audit | Redaction by default + opt-in text capture | Low (user-controlled) |
| **Network compromise** | Attacker intercepts auto-update | Signed release artifacts + hash verification | Low (signed updates) |
| **Supply chain attack** | Malicious dependency | Dependency audit via `cargo audit` + pinned versions | Medium (industry-wide risk) |

### Risk Classes

Ghost Guard classifies operations into risk levels:

| Class | Example | Policy |
|-------|---------|--------|
| **Allow** | Move file within Zone | Execute if approved |
| **Warn** | Batch >50 files; low-confidence classification | Highlight for review; allow if approved |
| **RequireApproval** | Overwrite existing file; move financial docs | Block without explicit confirmation |
| **Block** | Delete file; move outside Zone; modify system dirs | Reject, log block reason |

## Data Handling

### What Ghost Collects (By Default: Nothing)

Ghost does NOT collect:
- File contents
- Full file paths (only relative to Zone)
- Personal data
- Screen captures
- Keyboard input (only during explicit recording)
- Network traffic metadata

Ghost DOES collect (local, user-controlled):
- Workflow definitions (stored locally as JSON)
- Execution history (timestamps, file operations, success/failure)
- Audit logs (operation trace, policy decisions, hashes)
- Telemetry (only if opted in; feature usage, no PII)

### What Ghost Never Stores

- Passwords or secure-field input
- API keys or tokens
- Unredacted text from keyboard input
- Screenshots
- Window contents
- Browsing history

### Encryption

Workflows and audit logs can be encrypted at rest:
- **Key derivation**: Argon2id (password-based)
- **Cipher**: AES-256-GCM
- **Storage**: Local SQLite database, user-controlled

Encryption is **optional** (off by default). Users can enable via Settings → Security.

### User Control

Users can:
- Export audit logs as CSV or JSON at any time
- Delete all local data via Settings → Clear Data
- Revoke permissions (Zones, Capabilities) immediately
- Disable telemetry (off by default)
- Review and edit workflows before execution

## API Security

Ghost's Tauri backend is internal and **not** exposed to the network by default:

- All IPC is local (same machine)
- No remote API
- No cloud sync (local-first only)
- No authentication required (user is authenticated to OS)

Future cloud features (if added) will be:
- Explicitly opt-in
- End-to-end encrypted
- User-controlled sync targets only

## Release Security

### Code Signing

**macOS**:
- Developer ID certificate (signing in progress)
- Apple notarization (quarantine flag removal)
- Gatekeeper compatibility verification

**Windows**:
- EV code signing certificate (recommended)
- Standard code signing certificate (minimum)
- SmartScreen reputation

### Artifact Verification

Every release includes:
- SHA256 checksums (`checksums.txt`)
- Signed checksums (`checksums.txt.sig`)
- Provenance metadata (`provenance.json`)

Users can verify:
```bash
sha256sum -c checksums.txt
gpg --verify checksums.txt.sig  # With public key
```

### Dependencies

All Rust dependencies are audited:
- `cargo audit` in CI (blocks on vulnerabilities)
- `cargo deny check` for supply chain policy
- Dependabot automated updates
- Manual review of breaking changes

## Supported Versions

| Version | Status | Support Until |
|---------|--------|----------------|
| 1.1.x   | Current | TBD (first stable) |
| 1.0.x   | Legacy | End of month after 1.1.0 release |
| <1.0    | Unsupported | N/A |

**Security patches** are backported to the previous minor version only.

## Vulnerability Disclosure

We take security seriously. If you discover a vulnerability:

1. **Do not** open a public GitHub issue.
2. **Email** [security@ghost.example.com](mailto:security@ghost.example.com) with:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (optional)

3. We will acknowledge within 48 hours and aim to provide a fix within 7 days.

4. After a patch is released, you will be credited (unless you request anonymity).

**Scope**:
- Remote code execution
- Privilege escalation
- Unauthorized file access
- Audit log tampering
- Encryption bypass
- Cryptographic failures

**Out of scope**:
- Social engineering
- User approval bypass (if user explicitly approves a dangerous operation, that's user choice)
- Denial of service (DoS) via resource exhaustion
- Unpatched OS vulnerabilities

## Security in Development

### Pre-Commit Checks

Every commit runs:
- `cargo fmt --check` (format validation)
- `cargo clippy` (lint warnings as errors)
- `cargo test` (all 359 canonical tests)
- Secret scanning (gitleaks)

### CI Checks

Every push to main/PR runs:
- Full test suite on macOS / Windows / Linux
- Dependency audit (`cargo audit`)
- Clippy on all targets
- Format validation

### Manual Review

Security-critical changes require:
- Second review before merge
- Threat model update (if applicable)
- Test coverage for new attack surface

## Security Checklist

Before public release:

- [ ] SECURITY.md published and accessible
- [ ] Threat model documented
- [ ] All dependencies audited (`cargo audit` passes)
- [ ] Code signing certificates obtained and configured
- [ ] Release signing process tested
- [ ] Notarization process tested (macOS)
- [ ] All 359 tests passing
- [ ] No secrets in commit history
- [ ] Vulnerability disclosure process documented
- [ ] Security advisory template prepared
- [ ] Security headers configured (if applicable)

## Questions?

For security questions, email [security@ghost.example.com](mailto:security@ghost.example.com).

For bugs, use the GitHub issue tracker.

---

**Last updated**: 2026-07-06  
**Next review**: After Phase 3 completion (Week 8)
