# macOS Code Signing Checklist

This checklist covers the steps needed to code-sign and notarize Ghost for macOS distribution.

## Phase 1: Developer Account & Certificates (Before Release)

- [ ] Apple Developer Account (if not already created)
  - Cost: $99/year
  - Create at: https://developer.apple.com/account
  - Verify organization/identity

- [ ] Request Developer ID certificate
  - Type: Developer ID Application (for signing)
  - Request via: https://developer.apple.com/account/resources/certificates/list
  - Download and install locally: `~/Library/Keychains/login.keychain`
  - Verify: `security find-identity -v -p codesigning`

- [ ] Create App-Specific Password (for notarization)
  - https://appleid.apple.com/account/manage
  - Store securely (this is what we'll use in CI)
  - Create a unique password for "notarization"

- [ ] Enroll in Developer ID Notarization
  - https://developer.apple.com/account/resources/certificates/notarization
  - Notarization is now free (as of 2024)

## Phase 2: Local Testing (During Development)

- [ ] Test code signing locally
  ```bash
  codesign -s "Developer ID Application: Your Name (TEAM_ID)" \
    --deep --strict \
    Ghost.app
  ```

- [ ] Verify signature
  ```bash
  codesign -v -v Ghost.app
  ```

- [ ] Test Gatekeeper acceptance
  ```bash
  spctl -a -v Ghost.app
  # Should report: "accepted"
  ```

## Phase 3: Notarization Setup (Before First Release)

- [ ] Request Apple ID used for notarization
  - Should be Developer ID holder, or Admin
  - Will use App-Specific Password in CI

- [ ] Create notarization service account
  - Store credentials securely
  - Add to GitHub Secrets using the **exact names `.github/workflows/release.yml` reads** (these are the source of truth — the workflow enters full signing mode only when `BUILD_CERTIFICATE_BASE64` is present):
    - `BUILD_CERTIFICATE_BASE64` — Developer ID Application cert exported as `.p12`, base64-encoded
    - `P12_PASSWORD` — password for that `.p12`
    - `APPLE_SIGNING_IDENTITY` — e.g. `Developer ID Application: Your Name (TEAMID)`
    - `APPLE_ID` — Apple ID used for notarization
    - `APPLE_PASSWORD` — app-specific password for that Apple ID
    - `APPLE_TEAM_ID` — 10-character team ID

- [ ] Test notarization locally
  ```bash
  xcrun altool --notarize-app \
    -f Ghost.dmg \
    -t osx \
    --file-type dmg \
    -u $APPLE_ID \
    -p $APPLE_PASSWORD \
    --team-id $TEAM_ID
  ```

- [ ] Check notarization status
  ```bash
  xcrun altool --notarization-info <request-uuid> \
    -u $APPLE_ID \
    -p $APPLE_PASSWORD
  ```

- [ ] Staple notarization ticket
  ```bash
  xcrun stapler staple Ghost.dmg
  ```

## Phase 4: CI/CD Integration

Configured in `.github/workflows/release.yml`. Signing and notarization are handled
by the Tauri v2 bundler, not manual `altool` calls: the `Configure macOS signing`
step exports the secrets above as the env vars Tauri expects (`APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`,
`APPLE_TEAM_ID`), then `cargo tauri build --target universal-apple-darwin --bundles dmg`
imports the cert into a temp keychain, signs the app, and notarizes + staples the DMG.

- [ ] `BUILD_CERTIFICATE_BASE64` present → the run logs `SIGNING_MODE=full` (absent → `adhoc`, unsigned preview)
- [ ] Build creates a signed universal `.dmg`
- [ ] Tauri notarizes and staples the DMG when `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` are set
- [ ] `softprops/action-gh-release` publishes the DMG to the GitHub release

## Phase 5: Post-Release Verification

- [ ] Download .dmg from release
- [ ] Verify code signature
  ```bash
  codesign -v -v Ghost.app
  ```
- [ ] Verify Gatekeeper acceptance
  ```bash
  spctl -a -v Ghost.app
  ```
- [ ] Verify notarization ticket
  ```bash
  xcrun stapler validate Ghost.dmg
  ```

## Troubleshooting

### "Developer ID is not installed"
```bash
security find-identity -v -p codesigning
# If empty, download cert from Apple Developer portal
```

### "Notarization failed"
Check status:
```bash
xcrun altool --notarization-info <uuid> -u $APPLE_ID -p $APPLE_PASSWORD
```
Common issues: disk image too large, developer ID not recognized, expired credentials

### "Gatekeeper rejects the app"
Verify notarization was stapled:
```bash
xcrun stapler validate Ghost.dmg
```

## Resources

- [Apple Developer ID Documentation](https://developer.apple.com/support/developer-id/)
- [Notarizing macOS Software](https://developer.apple.com/documentation/notaryapi/notarizing_macos_software_before_distribution)
- [Gatekeeper Utilities](https://support.apple.com/en-us/106254)
- [codesign(1) Manual](https://www.unix.com/man-page/osx/1/codesign/)

## Timeline

| Step | Owner | Estimated Time | Deadline |
|------|-------|-----------------|----------|
| Get Developer Account | Founder | 1 day | Week 6 |
| Request Developer ID Cert | Founder | 1-3 days | Week 6 |
| Set up notarization credentials | Founder | 2 hours | Week 6 |
| Test locally | Founder | 2 hours | Week 6 |
| Integrate into release.yml | Engineer | 4 hours | Week 7 |
| Test full release pipeline | Engineer | 2 hours | Week 7 |
| First public notarized build | Release | 1 hour | Week 8 |

---

**Last updated**: 2026-07-06  
**Status**: Ready for Phase 3 (Week 6)
