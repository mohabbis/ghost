---
name: "#081 Sync GitHub Releases version with marketing site"
about: "P0 Task - Fix version confusion between releases and website"
title: "[P0] #081 Sync GitHub Releases version with marketing site (fix v2.0.3 gap)"
labels: "priority-0, yc-critical, release, versioning"
assignees: ""
---

## 🎯 Parent Epic
#080 [EPIC] Release Engineering: Fast, Safe, Transparent

## 📋 Task Description

Resolve the version mismatch between GitHub Releases and the marketing website. Users are confused about which version is latest and what features are included.

### Current Issues
- GitHub shows v2.0.2 as latest release
- Marketing website mentions v2.0.3
- No clear changelog for what changed between versions
- Download links inconsistent across platforms

### Tasks
- [ ] Audit all version references across:
  - GitHub Releases page
  - Marketing website (getghost.io or similar)
  - In-app version display
  - Download page
  - Documentation
- [ ] Decide on canonical version number (v2.0.3 or bump to v2.1.0?)
- [ ] Update all references to match
- [ ] Create missing release notes for any skipped versions
- [ ] Add automated version sync check to CI

## ✅ Acceptance Criteria

- [ ] Single source of truth for version (likely `Cargo.toml`)
- [ ] All public-facing materials show consistent version
- [ ] Clear changelog from v2.0.0 to current
- [ ] Automated check to prevent future drift
- [ ] Blog post or announcement clarifying the situation (if needed)

## 🔗 Related Issues
- Parent: #080
- Related: #082 (Technical Preview badge), #083 (checksums)

## ⏱️ Effort Estimate
**Time:** 0.5 days  
**Complexity:** Low  
**Risk:** Low (documentation/update work)

## 📝 Notes
This is embarrassing but fixable quickly. Better to acknowledge and fix than let confusion persist. Consider adding a "What's New" modal in-app to highlight recent changes.
