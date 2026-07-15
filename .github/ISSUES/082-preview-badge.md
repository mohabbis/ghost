---
name: "#082 Add Technical Preview badge to pre-release builds"
about: "P1 Task - Clearly mark experimental builds"
title: "[P1] #082 Add 'Technical Preview' badge to pre-release builds + in-app version display"
labels: "priority-1, release, ux"
assignees: ""
---

## 🎯 Parent Epic
#080 [EPIC] Release Engineering: Fast, Safe, Transparent

## 📋 Task Description

Add clear visual indicators to distinguish between production releases and technical preview (pre-release) builds. This helps users understand stability expectations and provides feedback channels.

### Implementation Areas

#### GitHub Releases
- [ ] Add `🧪 Technical Preview` badge to pre-release titles
- [ ] Update release template with stability warnings
- [ ] Add "Known Issues" section to pre-release notes
- [ ] Include feedback link in pre-release descriptions

#### In-App Display
- [ ] Modify version display to show badge for pre-release builds
  - Production: `v2.1.0`
  - Pre-release: `v2.1.0-beta.1 🧪`
- [ ] Add tooltip explaining what "Technical Preview" means
- [ ] Show "Report Issue" button prominently in preview builds
- [ ] Add settings toggle to opt-in/out of preview updates

#### Download Page
- [ ] Separate download links for Stable vs. Preview
- [ ] Clear warnings about preview build limitations
- [ ] Changelog comparison between stable and preview

## ✅ Acceptance Criteria

- [ ] All pre-release builds clearly marked on GitHub
- [ ] In-app version display shows badge for previews
- [ ] Users can easily identify which channel they're on
- [ ] Feedback mechanism visible in preview builds
- [ ] Documentation updated explaining release channels

## 🔗 Related Issues
- Parent: #080
- Related: #081 (version sync), #006 (feature flags)

## ⏱️ Effort Estimate
**Time:** 0.5 days  
**Complexity:** Low  
**Risk:** Low

## 📝 Notes
Consider adopting semantic versioning with clear pre-release tags: `v2.1.0-beta.1`, `v2.1.0-rc.1`, etc.
