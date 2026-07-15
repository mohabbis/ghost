---
issue_id: 071
parent_epic: 070
priority: P1
status: ⚪ Todo
labels: [security, ci, rust, compliance]
---

# #071 Run cargo audit + cargo deny in CI; Publish Results Publicly

## 📋 Summary
Add automated Rust dependency security auditing to CI pipeline with `cargo-audit` and `cargo-deny`, and publish results publicly for transparency.

## 🎯 Why This Matters
- **Security**: Catch vulnerable dependencies before they reach users
- **Trust**: Public audit results demonstrate security commitment
- **Compliance**: Required for SOC 2 readiness (#076)
- **YC demo**: Shows professional security practices

## ✅ Acceptance Criteria
- [ ] `cargo-audit` runs on every PR and main branch commit
- [ ] `cargo-deny` checks license compliance + advisories
- [ ] Build fails on critical/high severity advisories
- [ ] Audit results published to GitHub Security tab
- [ ] Public badge added to README showing audit status
- [ ] Documentation explains our security scanning approach

## 🔗 Related Issues
- Parent Epic: #070 (Security: Audit, Harden, Document)
- Related: #017 (cargo-deny in CI), #073 (integrity checks), #075 (Security Status page)

## 🛠️ Implementation Notes
### CI Configuration

**.github/workflows/security-audit.yml:**
```yaml
name: Security Audit

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
  schedule:
    - cron: '0 0 * * *'  # Daily at midnight

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
      
      - name: Install cargo-audit
        run: cargo install cargo-audit
      
      - name: Run cargo audit
        run: cargo audit --deny warnings
      
      - name: Install cargo-deny
        run: cargo install cargo-deny
      
      - name: Run cargo deny
        run: cargo deny check advisories licenses
      
      - name: Upload audit report
        uses: actions/upload-artifact@v4
        with:
          name: security-audit-report
          path: audit-report.json
```

### cargo-deny Configuration

**deny.toml:**
```toml
[advisories]
version = 2
yanked = "deny"
ignore = []  # Add specific advisory IDs if needed after review

[licenses]
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "ISC",
    "Unicode-DFS-2016",
]
deny = []
confidence-threshold = 0.8
```

### Public Transparency
- Add badge to README: `![Security Audit](https://github.com/ghost/ghost/actions/workflows/security-audit.yml/badge.svg)`
- Link to latest audit report in docs
- Consider publishing to https://deps.dev or similar

## 🧪 Testing Plan
- [ ] Introduce known vulnerable dependency in test branch
- [ ] Verify CI fails as expected
- [ ] Test ignore mechanism for false positives
- [ ] Verify artifact upload works
- [ ] Check GitHub Security tab integration

## ⏱️ Estimated Effort
**1 day**

## 📝 Definition of Done
- [ ] CI workflow created and tested
- [ ] cargo-deny configured
- [ ] Badge added to README
- [ ] Documentation updated
- [ ] Team trained on responding to audit failures

## 📊 Progress
- [ ] Workflow implementation
- [ ] Configuration tuning
- [ ] Testing
- [ ] Documentation
- [ ] Badge deployment
