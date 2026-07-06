# Ghost Threat Model

## Executive Summary

Ghost is a local-first desktop automation tool designed to be trustworthy by default. The threat model is built around **deny-by-default policy enforcement**, **explicit user approval**, **deterministic execution**, and **reversible operations**.

This document details the assets, threats, attack vectors, and mitigations.

## System Diagram

```
User Input (Intent)
        ↓
    [Planner] ← reads files, doesn't mutate
        ↓
    [Plan] (read-only proposal)
        ↓
    [Ghost Guard] (policy engine)
        ↓
    [Approval Gate] (user reviews & approves)
        ↓
    [Undo Journal Write] (before mutation)
        ↓
    [Executor] (performs approved actions)
        ↓
    [Audit Log] (immutable record)
        ↓
    [Undo Available] (user can reverse)
```

Every operation flows through all 7 stages. No shortcuts.

## Assets & Criticality

### Primary Assets

| Asset | Type | Criticality | Value |
|-------|------|-------------|-------|
| User files (Documents, Downloads, etc.) | Data | CRITICAL | Financial, personal, work data |
| Financial documents | Data | CRITICAL | Bank statements, invoices, receipts |
| Client/personal information | Data | CRITICAL | PII, business confidential |
| Audit logs | Data | HIGH | Compliance, accountability |
| Workflows | Code | MEDIUM | Business logic, automation intent |
| Undo journals | Data | MEDIUM | Recovery, reversibility |
| Session state | Data | LOW | Temporary, ephemeral |

### Secondary Assets

| Asset | Type | Criticality | Value |
|-------|------|-------------|-------|
| Release artifacts | Code | HIGH | Distribution, integrity |
| Update channel | Distribution | MEDIUM | Availability, integrity |
| Source code | Code | MEDIUM | Intellectual property |

## Threat Scenarios

### T1: Accidental File Loss

**Scenario**: User approves a plan without carefully reading it. Ghost moves/renames the wrong files.

**Attack vector**: User error (not attacker action).

**Impact**:
- Files moved to wrong locations
- Files renamed incorrectly
- Files scattered across directories
- Workflow unusable until fixed

**Mitigation**:
- Before/after tree preview (user sees exactly what changes)
- File count summary ("43 files to move, 31 to rename")
- Approval requires explicit interaction (not auto-approved)
- Low-confidence classifications flagged with ⚠️
- Undo available immediately after execution

**Residual risk**: User ignores preview and approves blindly. **Mitigated by UX education**.

---

### T2: Silent Overwrite

**Scenario**: Plan proposes moving file A to location where file B already exists. Executor overwrites B without user knowledge.

**Attack vector**: Ghost logic error or policy bypass.

**Impact**:
- File loss (B is replaced)
- Data corruption
- Audit trail becomes misleading

**Mitigation**:
- Conflict detection during planning (identifies all overwrites)
- Policy rule: Overwrite requires `RequireApproval` (blocks by default)
- Executor checks again before mutating
- Undo journal includes conflict info
- Audit log records overwrite attempt

**Residual risk**: LOW. Conflict detection runs twice (plan + execution).

---

### T3: Unauthorized Out-of-Zone File Access

**Scenario**: Workflow is tricked into moving files outside the approved Zone (e.g., `/Users/alice/Downloads` → `/System/Library`).

**Attack vector**:
- Malicious workflow crafted with path traversal (`../`)
- Path manipulation in workflow definition
- Symlink pointing outside Zone

**Impact**:
- System files could be modified
- Files could be moved to restricted directories
- Bypass of user-defined Zone boundaries

**Mitigation**:
- Zone enforcement at policy layer (deny-by-default)
- Path sanitization (reject `../`, absolute paths outside Zone)
- Symlink resolution before operation
- Policy engine checks every operation:
  ```
  if !path.starts_with(zone_path) {
      return Policy::Block("Outside Zone")
  }
  ```
- Audit log records all blocked operations

**Residual risk**: LOW. Defense-in-depth: sanitization + policy layer.

---

### T4: Audit Log Tampering

**Scenario**: Attacker gains local machine access and modifies audit log to cover tracks.

**Attack vector**:
- Direct file system access (requires user-level or admin access)
- SQLite database manipulation
- Hash collision (cryptographic failure)

**Impact**:
- Audit trail becomes untrustworthy
- Evidence of unauthorized operations disappears
- Compliance violations (if regulation requires immutable logs)

**Mitigation**:
- Hash-chained entries (each entry contains hash of previous entry)
- Tamper detection: if any entry is modified, subsequent hashes break
- Append-only design (no deletion or in-place modification)
- User can export and backup audit logs to external storage
- Immutability at SQLite schema level (only INSERT, no UPDATE/DELETE on audit table)

**Residual risk**: MEDIUM. Attacker with filesystem access can delete entire audit log. **Mitigated by**: user education + regular backups.

---

### T5: Undo Journal Corruption

**Scenario**: Undo journal becomes corrupted or incomplete, and user tries to undo a run.

**Attack vector**:
- Disk failure during journal write
- Partial write (power loss mid-operation)
- Bitrot / filesystem corruption

**Impact**:
- Undo fails or partially reverses
- Files left in inconsistent state
- Manual recovery required

**Mitigation**:
- Journal written atomically before execution (all-or-nothing)
- Hash validation before undo (detects corruption)
- Partial-undo detection (undo stops if it detects inconsistency)
- Audit log records undo success/failure
- User is informed if undo cannot complete

**Residual risk**: LOW. Atomic write + hash validation. Failure is detected and reported.

---

### T6: Malicious Workflow Definition

**Scenario**: User opens a workflow file that contains malicious operations (e.g., crafted JSON with path traversal).

**Attack vector**:
- User downloads workflow from untrusted source
- Workflow file is modified between save and load
- Social engineering (user tricks to approving dangerous workflow)

**Impact**:
- Unauthorized file operations
- Data loss
- Files moved to unintended locations

**Mitigation**:
- User must explicitly approve plan before execution
- Policy engine evaluates every operation (no trust in workflow definition)
- Approval shows before/after preview
- User can inspect workflow definition (JSON is human-readable)
- Audit trail records workflow source

**Residual risk**: MEDIUM. User can still be socially engineered into approving a dangerous operation. **Mitigated by**: explicit approval + clear preview.

---

### T7: Compromised Dependency

**Scenario**: A transitive dependency has a security vulnerability (e.g., RCE).

**Attack vector**:
- Attacker publishes malicious version to crates.io
- Supply chain compromise (dependency is already compromised upstream)

**Impact**:
- Remote code execution
- System compromise
- Data exfiltration

**Mitigation**:
- `cargo audit` in CI (blocks on known CVEs)
- `cargo deny check` for supply chain policy
- Dependabot automated updates + security alerts
- Dependency pinning (versions locked)
- Manual review of breaking changes
- Minimal dependency set (fewer attack vectors)

**Residual risk**: MEDIUM (industry-wide risk). **Mitigated by**: regular audits and updates.

---

### T8: Release Tampering

**Scenario**: Attacker intercepts binary download and replaces it with malicious version.

**Attack vector**:
- Man-in-the-middle (unencrypted download)
- Compromised GitHub release
- Compromised CDN

**Impact**:
- Malware distribution
- System compromise
- Data theft

**Mitigation**:
- HTTPS-only downloads (encryption in transit)
- Release checksums published and signed
- User can verify: `sha256sum -c checksums.txt`
- Code-signed binaries (macOS notarization, Windows signing)
- Gatekeeper verification (macOS)
- SmartScreen reputation (Windows)

**Residual risk**: LOW. Multi-layer verification (HTTPS + checksums + code signing).

---

### T9: Privilege Escalation

**Scenario**: User runs Ghost with elevated privileges (`sudo` or admin). Attacker exploits vulnerability to gain system-level access.

**Attack vector**:
- Ghost running as root/admin
- Vulnerability in Ghost code
- Vulnerability in Tauri framework

**Impact**:
- System compromise
- Arbitrary file access
- Installation of rootkits / malware

**Mitigation**:
- **Not recommended to run as admin** — Ghost shouldn't need elevated privileges
- Warn in UI if running as admin
- Request minimal permissions (Accessibility, Input Monitoring on macOS)
- No automatic privilege escalation
- Code review for unsafe operations

**Residual risk**: MEDIUM (user choice). **Mitigated by**: education and warnings.

---

### T10: Unattended Execution

**Scenario**: User leaves Ghost running, and it automatically executes workflows without user oversight.

**Attack vector**:
- Scheduled execution
- Background automation
- Script automation

**Impact**:
- Unintended file operations
- Data loss
- No user control

**Mitigation**:
- **Unattended execution is not a current feature**
- All execution requires explicit user approval (no auto-run)
- Replay is always interactive and interruptible

**Residual risk**: NONE (not implemented).

---

### T11: Sensitive Data in Logs

**Scenario**: Audit log captures PII, financial data, or secrets.

**Attack vector**:
- User types password during recording (captured in events)
- File paths contain PII
- Financial data in filenames

**Impact**:
- Information disclosure
- Compliance violation (GDPR, PCI-DSS, etc.)
- Privacy breach

**Mitigation**:
- **Secure fields suppressed at capture time** (passwords, tokens never stored)
- **Text capture is opt-in** (off by default)
- **File paths are relative** (only relative to Zone, not full paths)
- **Redaction available** (user can redact before export)
- **Audit logs are local** (user-controlled)
- User can delete audit log at any time

**Residual risk**: LOW. Secure fields never captured; user has full control of logs.

---

### T12: Symlink Attack

**Scenario**: Attacker creates a symlink pointing outside the Zone. Ghost follows the symlink and operates on the target.

**Attack vector**:
- Symlink in a monitored folder
- Ghost doesn't resolve symlinks before checking Zone

**Impact**:
- Out-of-Zone file operations
- Arbitrary file access
- Zone bypass

**Mitigation**:
- Symlink resolution before Zone check
- Real path validation (follow symlink, then check Zone)
- Policy engine blocks if resolved path is outside Zone

**Residual risk**: LOW. Defense-in-depth: symlink resolution + policy check.

---

## Policy Engine Verification

All operations are evaluated by Ghost Guard:

```rust
fn evaluate_operation(op: &Operation, zone: &Zone) -> PolicyDecision {
    // 1. Check Zone boundary
    if !op.target.is_inside(zone.path) {
        return Block("Outside Zone")
    }
    
    // 2. Check Capability
    if !zone.has_capability(op.kind) {
        return Block("Capability not granted")
    }
    
    // 3. Risk classification
    match op.kind {
        Delete => Block("Delete not allowed by default"),
        Overwrite => RequireApproval,
        Move => Allow,
        Rename => Allow,
        CreateFolder => Allow,
    }
}
```

**Guarantee**: All operations must pass through this gate. No exceptions.

## Incident Response

### If a Security Vulnerability is Found

1. **Report** via security@ghost.example.com (confidential)
2. **Acknowledge** within 48 hours
3. **Patch** within 7 days (target)
4. **Release** patched version
5. **Credit** vulnerability reporter (if desired)

### If a Breach Occurs

1. **Immediate**: Notify all affected users
2. **Within 24 hours**: Publish security advisory
3. **Within 72 hours**: Publish patch
4. **Post-incident**: Publish RCA (root cause analysis)

## Security Testing

### Manual Testing

- [ ] Path traversal attempts blocked
- [ ] Symlinks handled safely
- [ ] Out-of-Zone operations blocked
- [ ] Overwrite conflicts detected
- [ ] Audit logs are immutable
- [ ] Undo journals validate correctly
- [ ] Secure fields are never retained

### Automated Testing

- `cargo audit` (dependency vulnerabilities)
- `cargo deny check` (supply chain policy)
- `cargo test` (all 359 tests, including policy verification)
- Fuzzing (JSON workflow deserialization)
- Secret scanning (gitleaks, pre-commit hooks)

## Future Threat Mitigations

### Cloud Sync (if added)

- End-to-end encryption (user holds keys)
- Opt-in only (never forced)
- User-controlled sync targets
- Immutable audit logs in cloud
- Regular integrity verification

### Recorded Workflows (if added)

- Same trust pipeline as Organizer
- Semantic targeting (not coordinates)
- Target resolution tracing
- Replay-specific threat model review

### AI/ML Features (if added)

- Deterministic code path for execution (AI proposes only)
- Policy engine evaluates all AI suggestions
- User approval gate always present
- Opt-in feature (never enabled by default)

## Compliance

Ghost is designed to support:
- **GDPR** (local data, user control, export/delete)
- **SOC 2** (audit logs, access controls, incident response)
- **HIPAA** (local-first, no cloud by default)
- **PCI-DSS** (no secret retention, policy enforcement)

Compliance certifications are not yet pursued but are architecturally possible.

## References

- [OWASP Threat Modeling](https://owasp.org/www-community/Threat_Model)
- [CWE/SANS Top 25](https://cwe.mitre.org/top25/)
- [NIST Cybersecurity Framework](https://www.nist.gov/cyberframework/)
- [Trust model in AGENTS.md](AGENTS.md) — for developers

---

**Version**: 1.0  
**Last updated**: 2026-07-06  
**Next review**: After Phase 3 completion
