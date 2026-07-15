---
issue_id: 041
parent_epic: 040
priority: P1
status: ⚪ Todo
labels: [ux, onboarding, yc-critical]
---

# #041 Redesign Onboarding: "What Ghost Does" in <60 Seconds

## 📋 Summary
Create an interactive onboarding flow that demonstrates Ghost's core value proposition (trust pipeline) in under 60 seconds with hands-on demo.

## 🎯 Why This Matters
- **First impressions**: YC judges and new users must "get it" immediately
- **Trust clarity**: Core differentiator is `Approve → Execute → Audit → Undo`
- **Activation**: Users who complete onboarding 3x more likely to retain
- **Simplicity**: Counteract "another automation tool" assumption

## ✅ Acceptance Criteria
- [ ] New user sees interactive demo on first launch
- [ ] Demo completes in <60 seconds
- [ ] User performs one real action during demo (e.g., approve a safe workflow)
- [ ] Trust pipeline visualization shown clearly
- [ ] Option to skip for advanced users
- [ ] Onboarding can be re-triggered from settings

## 🔗 Related Issues
- Parent Epic: #040 (UX: Reduce Cognitive Load, Highlight Trust)
- Related: #042 (Mode Selector), #043 (Trust pipeline visualization)

## 🛠️ Implementation Notes
### Flow Design

**Screen 1 (5s):** "Ghost automates your computer—safely"
- Brief tagline + illustration

**Screen 2 (15s):** Interactive demo setup
- Create a harmless demo workflow (e.g., "Open Calculator" or "List files in Downloads")
- Show the semantic plan before execution

**Screen 3 (20s):** User approves
- Highlight the "Inspect → Approve" step
- Show what will happen in plain English

**Screen 4 (10s):** Execution + audit
- Run the action
- Show it appear in audit log
- Emphasize "You approved this"

**Screen 5 (10s):** "You're ready"
- Link to main Organizer view
- Offer to explore sample workflows

### Technical Approach
- Store `onboarding_completed: boolean` in redb
- Demo workflow runs in sandboxed mode
- Use existing approval/execution pipeline (dogfood our trust model)

## 🧪 Testing Plan
- [ ] Usability test with 3 people unfamiliar with Ghost
- [ ] Time each step (must be <60s total)
- [ ] Verify comprehension: ask users to explain Ghost after
- [ ] Test skip functionality
- [ ] Test re-onboarding trigger

## ⏱️ Estimated Effort
**3 days**

## 📝 Definition of Done
- [ ] Onboarding flow implemented
- [ ] Demo workflow created
- [ ] Timing validated (<60s)
- [ ] Usability tested
- [ ] Documentation updated

## 📊 Progress
- [ ] Flow design
- [ ] UI implementation
- [ ] Demo workflow
- [ ] Testing
- [ ] Polish
