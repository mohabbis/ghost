# GitHub branch protection rules

# This file documents the branch protection rules for Ghost.
# Apply these settings manually in GitHub Settings → Branches → main

## Main branch protection rules

- **Branch name pattern**: `main`

### Require status checks to pass before merging

- ✅ Required checks:
  - `build` (rust.yml)
  - `test` (rust.yml)
  - `clippy` (rust.yml)
  - `format` (rust.yml)
  - `security` (security.yml)
  - `secret-scanning` (security.yml)
  - `dependency-audit` (security.yml)

### Require code reviews before merging

- Number of required reviewers: **1**
- Dismiss stale PR approvals: **Yes**
- Require review from code owners: **Yes**
- Require approval of the most recent reviewable push: **Yes**

### Other protections

- **Allow force pushes**: No
- **Allow deletions**: No
- **Require branches to be up to date before merging**: Yes
- **Require conversation resolution before merging**: Yes
- **Require signed commits**: No (recommended for future)

## Develop branch protection (relaxed)

- **Branch name pattern**: `develop`
- Require 1 approved review
- Require CI to pass
- Allow force pushes by admins (for cleanup)
- Allow deletions

## Release branch (stricter)

- **Branch name pattern**: `release/*`
- Require 2 approved reviews (code signing authority)
- Require all CI checks
- Require signed commits (future)
- No force pushes
- No deletions

## Tag protection

- **Pattern**: `v*` (semantic versioning)
- Require: Release manager approval
- Enforce: Signed tags (future)

---

**Note**: These are recommendations. Apply via GitHub Settings → Branches.
