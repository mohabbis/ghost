---
name: "#006 Add Upgrade Mode feature flag"
about: "P0 Task - Enable safe parallel testing of new stack"
title: "[P0] #006 Add 'Upgrade Mode' feature flag to safely test new stack alongside old"
labels: "priority-0, yc-critical, feature-flag, infrastructure"
assignees: ""
---

## 🎯 Parent Epic
#001 [EPIC] Tech Stack Upgrade: Strategy & Non-Goals

## 📋 Task Description

Implement a feature flag system that allows users to opt-in to the new tech stack while keeping the old stack as the default. This enables safe A/B testing and gradual rollout.

### Feature Flag Requirements

#### Implementation
- [ ] Create `FeatureFlag` struct in Rust backend
- [ ] Add `upgrade_mode` boolean flag (default: `false`)
- [ ] Store flag state in user preferences (redb)
- [ ] Expose flag via Tauri invoke API to frontend
- [ ] Add UI toggle in Settings > Advanced (hidden behind dev mode initially)

#### Behavior When Enabled
- Frontend loads new TypeScript/Svelte components (if available)
- Backend uses upgraded async runtime paths
- All data writes to same redb storage (backward compatible)
- Audit logs marked with `stack_version` for analysis

#### Behavior When Disabled (Default)
- Frontend loads existing vanilla JS components
- Backend uses current stable paths
- No visible changes to user experience

#### Safety Mechanisms
- [ ] Automatic rollback if new stack crashes 3+ times in 5 minutes
- [ ] One-click disable button always accessible
- [ ] Clear warning message about experimental nature
- [ ] Telemetry to track adoption and stability (opt-in only)

## ✅ Acceptance Criteria

- [ ] Feature flag infrastructure implemented
- [ ] UI toggle available in Settings > Advanced
- [ ] Automatic crash detection and rollback
- [ ] Documentation for users on how to enable/disable
- [ ] Telemetry events for flag state changes (anonymized)

## 🔗 Related Issues
- Parent: #001
- Related: #032 (TypeScript migration), #082 (Technical Preview badge)

## ⏱️ Effort Estimate
**Time:** 1 day  
**Complexity:** Medium  
**Risk:** Medium (need robust rollback mechanism)

## 📝 Notes
This is the critical enabler for zero-downtime upgrades. Users should be able to toggle back anytime without data loss.
