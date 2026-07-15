---
issue_id: 011
parent_epic: 010
priority: P1
status: 🟠 In Progress
labels: [rust, tauri, backend, yc-critical]
---

# #011 Upgrade Tauri to Latest Stable 2.x

## 📋 Summary
Upgrade Tauri framework from current version to latest stable 2.x release and audit all breaking changes in the plugin API.

## 🎯 Why This Matters
- **Security**: Latest Tauri includes critical security patches
- **Performance**: 2.x brings improved IPC and reduced bundle size
- **Compatibility**: Required for accessing new native APIs (Shortcuts, menu bar enhancements)
- **YC Demo**: Shows we're on modern, maintained stack

## ✅ Acceptance Criteria
- [ ] Tauri upgraded to 2.1+ in `Cargo.toml`
- [ ] All plugin APIs audited for breaking changes
- [ ] Build passes on macOS (Apple Silicon + Intel)
- [ ] Build passes on Windows
- [ ] All existing features work post-upgrade (regression test)
- [ ] Documentation updated with migration notes
- [ ] Changelog entry created

## 🔗 Related Issues
- Parent Epic: #010 (Rust Backend: Stability + Performance)
- Related: #012 (reqwest upgrade), #081 (version sync)

## 🛠️ Implementation Notes
1. Check [Tauri 2.x migration guide](https://v2.tauri.app/start/migrate/from-tauri-1/)
2. Key breaking changes to watch:
   - Plugin registration API changes
   - Window configuration schema updates
   - IPC command handler signature changes
3. Test all native integrations:
   - Accessibility permissions
   - Menu bar behavior
   - System tray (if used)

## 🧪 Testing Plan
- [ ] Manual: Launch app, verify all views render
- [ ] Manual: Trigger Organizer scan, verify execution
- [ ] Manual: Test approval workflow end-to-end
- [ ] Automated: Run existing Rust test suite
- [ ] Edge case: Test with old workflow files (backward compat)

## ⏱️ Estimated Effort
**2 days**

## 📝 Definition of Done
- [x] Code complete
- [ ] Tests passing
- [ ] Documentation updated
- [ ] Review completed
- [ ] Tested on both macOS and Windows

## 🚨 Blockers
None currently

## 📊 Progress
- [x] Initial research
- [ ] Dependency update
- [ ] Breaking change audit
- [ ] Testing
- [ ] Documentation
