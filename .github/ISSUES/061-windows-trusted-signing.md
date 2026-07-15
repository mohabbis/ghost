---
issue_id: 061
parent_epic: 060
priority: P2
status: ⚪ Todo
labels: [windows, security, signing, trust]
---

# #061 Implement Azure Trusted Signing for Windows Installer

## 📋 Summary
Replace current code signing approach with Azure Trusted Signing to eliminate Windows SmartScreen warnings and establish trust on Windows platforms.

## 🎯 Why This Matters
- **Trust gap**: SmartScreen warnings scare new Windows users
- **Professionalism**: Unsigned/unknown publisher looks suspicious
- **YC demo**: Windows users should have same trust experience as macOS
- **Security**: EV-style signing without hardware token complexity

## ✅ Acceptance Criteria
- [ ] Azure Trusted Signing configured in CI/CD
- [ ] Windows installer signed with valid certificate
- [ ] SmartScreen shows "Ghost" as verified publisher (no warning)
- [ ] Signing integrated into GitHub Actions workflow
- [ ] Certificate renewal process documented
- [ ] Cost tracked (Azure Trusted Signing is pay-per-signature)

## 🔗 Related Issues
- Parent Epic: #060 (Windows: Trust + Reliability)
- Related: #073 (integrity checks), #083 (checksums + signatures)

## 🛠️ Implementation Notes
### Azure Trusted Signing Setup

**Prerequisites:**
1. Azure subscription (use existing or create new)
2. Trusted Signing resource created
3. Code signing certificate requested

**CI/CD Integration:**
```yaml
# GitHub Actions step example
- name: Sign Windows installer
  uses: azure/trusted-signing-action@v1
  with:
    azure-tenant-id: ${{ secrets.AZURE_TENANT_ID }}
    client-id: ${{ secrets.AZURE_CLIENT_ID }}
    client-secret: ${{ secrets.AZURE_CLIENT_SECRET }}
    endpoint: ${{ secrets.TRUSTED_SIGNING_ENDPOINT }}
    certificate-id: ${{ secrets.CERTIFICATE_ID }}
    files: ghost-installer.exe
```

**Cost Estimate:**
- ~$0.10-0.50 per signature (depending on volume)
- Budget: ~$50/month for frequent releases

### Alternative Considered
- **Traditional EV Code Signing**: Requires hardware token, more expensive upfront ($500+), harder to automate
- **Azure Trusted Signing**: Cloud-based, pay-per-use, CI/CD friendly ✅

## 🧪 Testing Plan
- [ ] Download signed installer on clean Windows VM
- [ ] Verify SmartScreen shows verified publisher
- [ ] Test on Windows 10 + Windows 11
- [ ] Test on Windows ARM64 (if available)
- [ ] Verify signature with `signtool verify`

## ⏱️ Estimated Effort
**2-3 days** (includes Azure setup + testing)

## 📝 Definition of Done
- [ ] Azure Trusted Signing configured
- [ ] CI/CD pipeline updated
- [ ] Test build signed successfully
- [ ] SmartScreen verified (no warnings)
- [ ] Documentation updated
- [ ] Renewal process documented

## 🚨 Blockers
- Requires Azure subscription setup
- May need finance approval for billing

## 📊 Progress
- [ ] Azure subscription ready
- [ ] Trusted Signing resource created
- [ ] Certificate issued
- [ ] CI/CD integration
- [ ] Testing
- [ ] Production rollout
