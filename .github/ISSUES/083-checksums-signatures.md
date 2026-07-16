---
name: "#083 Automate SHA256 checksums + GPG signatures"
about: "P1 Task - Provide verifiable download integrity"
title: "[P1] #083 Automate SHA256 checksums + GPG signature verification instructions"
labels: "priority-1, security, release, automation"
assignees: ""
---

## 🎯 Parent Epic
#080 [EPIC] Release Engineering: Fast, Safe, Transparent

> **Status (2026-07-16): implemented — with cosign keyless instead of GPG.**
> `release.yml` generates `SHA256SUMS.txt` over all artifacts, renders the hash
> table into the release body, and now signs `Ghost.dmg`, `Ghost_Setup.exe`,
> and `SHA256SUMS.txt` itself with Sigstore cosign keyless signing (GitHub
> OIDC → Fulcio certificate; `.cosign.sig`/`.cosign.pem` attached per
> artifact). This deliberately supersedes the GPG approach below: there is no
> maintainer-held key to provision, store, or rotate, which this issue's own
> notes suggested ("consider sigstore/cosign"). Verification instructions:
> `docs/VERIFY_DOWNLOADS.md`, linked from the README download section and each
> release body. Remaining (optional) items are marked below.

## 📋 Task Description

Automate the generation and publication of SHA256 checksums and GPG signatures for all release artifacts. This allows users to verify downloads haven't been tampered with.

### Implementation

#### CI/CD Automation
- [x] Add checksum generation to release workflow *(release.yml "Generate SHA256SUMS" step)*
  - Generate SHA256 for all artifacts (.exe, .dmg, .deb, etc.)
  - Publish `SHA256SUMS` file alongside release assets
- [x] Add signing step *(done via sigstore/cosign keyless, not GPG — release.yml "Sign release artifacts")*
  - Sign all release artifacts ~~with project GPG key~~ → cosign keyless (GitHub OIDC)
  - Sign `SHA256SUMS` file itself
  - ~~Store GPG key in GitHub Secrets~~ (keyless: no stored key at all)

#### Documentation
- [x] Create `/docs/VERIFY_DOWNLOADS.md` with: *(created 2026-07-16; cosign commands instead of GPG)*
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
- [x] Add verification instructions to every GitHub Release *(release body links docs/VERIFY_DOWNLOADS.md)*

#### User Experience
- [ ] Add "Verify Download" link on download page
- [ ] Consider in-app verification check on first launch
- [ ] Display fingerprint of signing key prominently

## ✅ Acceptance Criteria

- [x] All releases automatically generate checksums
- [x] All releases signed *(cosign keyless, superseding GPG)*
- [x] Clear verification documentation exists (`docs/VERIFY_DOWNLOADS.md`)
- [ ] Verification process tested end-to-end *(runs on the next published v* tag — signing steps cannot execute outside a tagged release build)*
- [x] ~~Public key easily accessible~~ *(keyless: identity is the repo's release workflow, checked via `--certificate-identity-regexp`)*

## 🔗 Related Issues
- Parent: #080
- Related: #071 (security audit), #073 (integrity checks)

## ⏱️ Effort Estimate
**Time:** 1 day  
**Complexity:** Medium  
**Risk:** Low (but high impact on trust)

## 📝 Notes
Consider exploring sigstore/cosign for keyless signing as a modern alternative to GPG. Either way, make verification dead simple for users.
