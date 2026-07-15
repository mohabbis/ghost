---
issue_id: 073
parent_epic: 070
priority: P1
status: ⚪ Todo
labels: [security, integrity, workflows]
---

# #073 Add Integrity Checks for Downloaded Workflows/Plugins

## 📋 Summary
Implement SHA-256 checksum verification and optional signature validation for all downloaded workflows and plugins to prevent tampering.

## 🎯 Why This Matters
- **Supply chain security**: Prevent malicious workflow modifications
- **Trust**: Users can verify workflows haven't been altered
- **Compliance**: Required for enterprise security policies
- **YC demo**: Shows we take supply chain attacks seriously

## ✅ Acceptance Criteria
- [ ] All workflow downloads include SHA-256 checksum verification
- [ ] Checksums stored in manifest and verified before execution
- [ ] Optional GPG/signature verification for signed workflows
- [ ] UI shows verification status (✅ Verified / ⚠️ Unverified)
- [ ] Failed verification blocks execution with clear error
- [ ] Documentation explains verification to users

## 🔗 Related Issues
- Parent Epic: #070 (Security: Audit, Harden, Document)
- Related: #061 (Windows signing), #083 (checksums for releases)

## 🛠️ Implementation Notes
### Checksum Verification Flow

**Download:**
1. Fetch workflow file + checksum file (or embedded checksum)
2. Compute SHA-256 of downloaded content
3. Compare against expected checksum
4. Reject if mismatch

**Manifest Format:**
```json
{
  "workflow_id": "example-workflow",
  "version": "1.0.0",
  "checksum": "sha256:abc123...",
  "signature": "optional-gpg-signature",
  "signed_by": "optional-key-id"
}
```

**Rust Implementation:**
```rust
use sha2::{Sha256, Digest};

fn verify_checksum(content: &[u8], expected: &str) -> Result<(), Error> {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let computed = hex::encode(hasher.finalize());
    
    if computed == expected {
        Ok(())
    } else {
        Err(Error::ChecksumMismatch)
    }
}
```

### Signature Verification (Optional)
For workflows signed by trusted authors:
- Support minikeys/pgp signatures
- Store public keys in trust store
- Verify signature after checksum passes

### UI Indicators
- ✅ **Verified**: Checksum + signature valid
- ⚠️ **Unverified**: Checksum valid, no signature
- ❌ **Failed**: Checksum mismatch (BLOCKED)

## 🧪 Testing Plan
- [ ] Test with valid checksum (passes)
- [ ] Test with corrupted file (blocked)
- [ ] Test with missing checksum (policy decision needed)
- [ ] Test signature verification with valid key
- [ ] Test signature verification with invalid key
- [ ] Performance test: checksum overhead acceptable

## ⏱️ Estimated Effort
**2 days**

## 📝 Definition of Done
- [ ] Checksum verification implemented
- [ ] Signature verification (optional) working
- [ ] UI shows verification status
- [ ] Failed verification blocks execution
- [ ] Documentation updated
- [ ] Migration plan for existing workflows

## 📊 Progress
- [ ] Design verification scheme
- [ ] Implement checksum logic
- [ ] Add signature support
- [ ] UI integration
- [ ] Testing
- [ ] Documentation
