# Windows Code Signing Checklist

This checklist covers the steps needed to code-sign Ghost for Windows distribution.

## Phase 1: Code Signing Certificate (Before Release)

### Option A: Standard Code Signing Certificate (Recommended for MVP)

- [ ] Purchase Standard code signing certificate
  - Provider: Sectigo, DigiCert, Comodo, or similar
  - Cost: $200-400/year
  - Type: "Authenticode" or "Software Publisher"
  - Validity: 1-3 years

- [ ] Create Certificate Signing Request (CSR)
  ```bash
  openssl req -new -newkey rsa:2048 -nodes \
    -out ghost.csr \
    -keyout ghost.key \
    -subj "/C=US/ST=State/L=City/O=Organization/CN=Ghost/emailAddress=contact@ghost.example.com"
  ```

- [ ] Submit CSR to CA
  - Provider will verify your identity
  - Provide documentation (driver's license, organization verification)
  - Typical approval time: 1-3 business days

- [ ] Download signed certificate
  - Save as `ghost.pfx` (PKCS#12, includes cert + private key)
  - Encrypt with strong password
  - Store securely (never commit to git)

### Option B: Extended Validation (EV) Certificate (For Enterprise)

- [ ] Purchase EV code signing certificate
  - Cost: $500-1000/year
  - Higher trust level (SmartScreen reputation faster)
  - Requires more thorough identity verification
  - Timeline: 3-7 days

- [ ] Follow same CSR + submission process
  - Additional documentation (business license, D&B verification)

## Phase 2: Local Signing Setup

- [ ] Install Windows SDK (includes `signtool.exe`)
  ```bash
  # On Windows:
  # Download from https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/
  # Or via choco: choco install windows-sdk
  ```

- [ ] Test signing locally (on Windows machine)
  ```bash
  signtool sign /f ghost.pfx /p <password> /t http://timestamp.server.com Ghost_Setup.exe
  ```

- [ ] Verify signature
  ```bash
  signtool verify /pa Ghost_Setup.exe
  ```

## Phase 3: Certificate Management

- [ ] Store certificate securely
  - Never commit .pfx to git
  - Add to `.gitignore`
  - Store in secure key management system (e.g., Azure Key Vault, 1Password)

- [ ] Add to GitHub Secrets (for CI/CD)
  ```
  WINDOWS_CODE_SIGNING_CERT_BASE64 = <base64-encoded .pfx>
  WINDOWS_CODE_SIGNING_CERT_PASSWORD = <password>
  ```

- [ ] Test CI integration
  - Decode cert from base64
  - Sign release executable
  - Verify signature in artifact

## Phase 4: Timestamping

**Important**: Always include a timestamp server when signing. This allows the signature to remain valid even after the certificate expires.

- [ ] Choose timestamp server
  - Sectigo (free): `http://timestamp.sectigo.com`
  - DigiCert (free): `http://timestamp.digicert.com`
  - GlobalSign (free): `http://timestamp.globalsign.com`

- [ ] Add to signing command
  ```bash
  signtool sign /f ghost.pfx /p <password> \
    /t http://timestamp.sectigo.com \
    Ghost_Setup.exe
  ```

## Phase 5: SmartScreen Reputation

- [ ] First release may show SmartScreen warning
  - "Windows SmartScreen couldn't find the publisher"
  - This is normal for new publishers

- [ ] Build reputation over time
  - Multiple signed releases
  - Low complaint rate
  - Active updates
  - Usually 2-4 weeks for reputation to build

- [ ] Monitor reputation
  - Watch for SmartScreen warnings in user feedback
  - Report issues to Microsoft if certificate is compromised

## Phase 6: CI/CD Integration

Configured in `.github/workflows/release.yml`:

- [ ] Decode certificate from base64
  ```bash
  echo ${{ secrets.WINDOWS_CODE_SIGNING_CERT_BASE64 }} | base64 -d > cert.pfx
  ```

- [ ] Sign executable
  ```bash
  signtool sign /f cert.pfx /p ${{ secrets.WINDOWS_CODE_SIGNING_CERT_PASSWORD }} \
    /t http://timestamp.sectigo.com \
    Ghost_Setup.exe
  ```

- [ ] Clean up certificate after signing
  ```bash
  rm cert.pfx
  ```

- [ ] Verify signature in artifact (optional)
  ```bash
  signtool verify /pa Ghost_Setup.exe
  ```

## Phase 7: Release Verification

- [ ] Download .exe from release
- [ ] Verify signature locally
  ```bash
  signtool verify /pa Ghost_Setup.exe
  ```
- [ ] Check SmartScreen behavior
  - First release: may show warning (normal)
  - Subsequent releases: should be trusted faster
- [ ] Monitor user feedback for SmartScreen issues

## Troubleshooting

### "Certificate password is incorrect"
- Verify password is correct when purchasing
- Check for special characters in password

### "Timestamp server is unreachable"
- Use different timestamp server from list above
- Check network connectivity

### "The certificate is invalid"
- Verify certificate file is valid: `certutil -dump ghost.pfx`
- Ensure certificate is not expired
- Ensure you're using correct password

### "SmartScreen still warns after signing"
- SmartScreen reputation takes time to build
- Multiple signed releases help
- Check for Microsoft reputation issues

## Cost-Benefit Analysis

| Type | Cost | Time | Reputation | Effort |
|------|------|------|-----------|--------|
| Standard | $200-400/yr | 1-3 days | Builds over time | Low |
| EV | $500-1000/yr | 3-7 days | Faster | Medium |
| Self-signed (not recommended) | $0 | 0 min | None (rejected by SmartScreen) | Zero |

**Recommendation**: Start with Standard certificate. Upgrade to EV if funding permits.

## References

- [Microsoft Authenticode Documentation](https://docs.microsoft.com/en-us/windows/win32/seccrypto/authenticode)
- [SignTool Documentation](https://docs.microsoft.com/en-us/windows/win32/seccrypto/signtool)
- [SmartScreen Reputation](https://docs.microsoft.com/en-us/windows/security/threat-protection/windows-defender-smartscreen/windows-defender-smartscreen-overview)

## Timeline

| Step | Owner | Estimated Time | Deadline |
|------|-------|-----------------|----------|
| Purchase certificate | Founder | 1 day | Week 6 |
| Create CSR + submit | Founder | 1 hour | Week 6 |
| Receive signed cert | CA | 1-3 days | Week 6 |
| Test signing locally | Engineer | 1 hour | Week 7 |
| Integrate into CI/CD | Engineer | 2 hours | Week 7 |
| Test full pipeline | Engineer | 1 hour | Week 7 |
| First signed release | Release | 30 min | Week 8 |

---

**Last updated**: 2026-07-06  
**Status**: Ready for Phase 3 (Week 6)
