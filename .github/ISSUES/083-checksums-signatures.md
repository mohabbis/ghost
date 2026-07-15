---
name: "#083 Automate SHA256 checksums + GPG signatures"
about: "P1 Task - Provide verifiable download integrity"
title: "[P1] #083 Automate SHA256 checksums + GPG signature verification instructions"
labels: "priority-1, security, release, automation"
assignees: ""
---

## 🎯 Parent Epic
#080 [EPIC] Release Engineering: Fast, Safe, Transparent

## 📋 Task Description

Automate the generation and publication of SHA256 checksums and GPG signatures for all release artifacts. This allows users to verify downloads haven't been tampered with.

### Implementation

#### CI/CD Automation
- [ ] Add checksum generation to release workflow
  - Generate SHA256 for all artifacts (.exe, .dmg, .deb, etc.)
  - Publish `SHA256SUMS` file alongside release assets
- [ ] Add GPG signing step
  - Sign all release artifacts with project GPG key
  - Sign `SHA256SUMS` file itself
  - Store GPG key in GitHub Secrets (or better: use sigstore)

#### Documentation
- [ ] Create `/docs/VERIFY_DOWNLOADS.md` with:
  - Step-by-step verification instructions for each platform
  - Import instructions for GPG public key
  - Example commands:
    ```bash
    # macOS/Linux
    sha256sum -c SHA256SUMS
    gpg --verify ghost-installer.dmg.sig ghost-installer.dmg
    
    # Windows (PowerShell)
    Get-FileHash ghost-installer.exe -Algorithm SHA256
    ```
- [ ] Add verification instructions to every GitHub Release

#### User Experience
- [ ] Add "Verify Download" link on download page
- [ ] Consider in-app verification check on first launch
- [ ] Display fingerprint of signing key prominently

## ✅ Acceptance Criteria

- [ ] All releases automatically generate checksums
- [ ] All releases signed with GPG key
- [ ] Clear verification documentation exists
- [ ] Verification process tested end-to-end
- [ ] Public key easily accessible (website, keybase, etc.)

## 🔗 Related Issues
- Parent: #080
- Related: #071 (security audit), #073 (integrity checks)

## ⏱️ Effort Estimate
**Time:** 1 day  
**Complexity:** Medium  
**Risk:** Low (but high impact on trust)

## 📝 Notes
Consider exploring sigstore/cosign for keyless signing as a modern alternative to GPG. Either way, make verification dead simple for users.
