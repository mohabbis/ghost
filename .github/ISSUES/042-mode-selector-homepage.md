---
issue_id: 042
parent_epic: 040
priority: P1
status: ⚪ Todo
labels: [ux, navigation, yc-critical]
---

# #042 Add "Mode Selector" Homepage

## 📋 Summary
Replace current single-view launch with a mode selector homepage that offers: Organizer (default), Record, Guard Desk, and Advanced modes.

## 🎯 Why This Matters
- **Progressive disclosure**: New users see simple path; power users find advanced features
- **Mental model clarity**: Each mode has distinct purpose
- **YC demo**: Shows thoughtful UX design, not just feature dump
- **Reduced cognitive load**: Users focus on one task at a time

## ✅ Acceptance Criteria
- [ ] Homepage shows 4 mode cards with icons + descriptions
- [ ] Organizer mode is default/recommended
- [ ] Each mode navigates to dedicated view
- [ ] Mode selection remembered per-user
- [ ] Quick switcher available in nav for returning users
- [ ] Mobile-responsive layout

## 🔗 Related Issues
- Parent Epic: #040 (UX: Reduce Cognitive Load, Highlight Trust)
- Related: #041 (Onboarding), #043 (Trust pipeline visualization)

## 🛠️ Implementation Notes
### Mode Cards

**📋 Organizer** *(Recommended)*
- "Browse and run community workflows"
- Icon: 📋
- Default for new users

**🎬 Record**
- "Capture your actions as a workflow"
- Icon: 🎬
- For creating custom automations

**🛡️ Guard Desk**
- "Review pending approvals and audit logs"
- Icon: 🛡️
- For managing trust pipeline

**⚙️ Advanced**
- "Direct access to all features + settings"
- Icon: ⚙️
- For power users

### Technical Approach
- Add `last_mode` to user preferences in redb
- Create route structure: `/home`, `/organizer`, `/record`, `/guard`, `/advanced`
- Preserve deep linking for direct mode access
- Add skip option: "Always go to Organizer" checkbox

## 🧪 Testing Plan
- [ ] Usability test: Can users find their intended mode?
- [ ] Navigation testing: All modes accessible
- [ ] Persistence test: Mode preference saved
- [ ] A/B test: Compare with old single-view (if time permits)

## ⏱️ Estimated Effort
**2 days**

## 📝 Definition of Done
- [ ] Homepage UI implemented
- [ ] All 4 modes functional
- [ ] Preference persistence works
- [ ] Navigation tested
- [ ] Documentation updated

## 📊 Progress
- [ ] Design mockups
- [ ] Component implementation
- [ ] Routing setup
- [ ] Persistence layer
- [ ] Testing
