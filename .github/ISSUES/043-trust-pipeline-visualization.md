---
issue_id: 043
parent_epic: 040
priority: P1
status: ⚪ Todo
labels: [ux, visualization, trust, yc-critical]
---

# #043 Visualize Trust Pipeline: Animated Flow

## 📋 Summary
Create an animated visualization showing the `Record → Inspect → Approve → Execute → Undo` pipeline to make Ghost's trust model tangible and memorable.

## 🎯 Why This Matters
- **Differentiation**: This is Ghost's core innovation—make it visible
- **Trust building**: Users understand exactly what happens to their data
- **YC demo**: Instantly communicates "why Ghost is different"
- **Education**: Reduces support questions about how approval works

## ✅ Acceptance Criteria
- [ ] Animation shows all 5 stages of trust pipeline
- [ ] Real workflow status flows through animation (live updates)
- [ ] Clicking each stage shows explanation + examples
- [ ] Animation plays during onboarding (see #041)
- [ ] Accessible: Works with screen readers, reduced motion preference
- [ ] Dark mode compatible

## 🔗 Related Issues
- Parent Epic: #040 (UX: Reduce Cognitive Load, Highlight Trust)
- Related: #041 (Onboarding), #042 (Mode Selector), #003 (Trust boundary tests)

## 🛠️ Implementation Notes
### Visualization Design

**Stage 1: Record** 🎬
- Icon: Recording dot pulsing
- Text: "Your actions are captured"
- Example: Mouse clicks, keystrokes logged

**Stage 2: Inspect** 🔍
- Icon: Magnifying glass over code
- Text: "Semantic plan generated"
- Example: Show plain English translation

**Stage 3: Approve** ✅
- Icon: Checkmark button
- Text: "You review and approve"
- Example: Highlight user control moment

**Stage 4: Execute** ⚡
- Icon: Lightning bolt
- Text: "Approved code runs"
- Example: Show action happening

**Stage 5: Undo** ↩️
- Icon: Undo arrow
- Text: "Reverse if needed"
- Example: Show audit log + reversal option

### Technical Approach
- Use CSS animations or lightweight SVG animation
- Connect to real workflow state machine
- Add tooltips with "Why this matters" explanations
- Store animation preferences in user settings

### Animation States
- **Idle**: Show full pipeline static
- **Active**: Highlight current stage as workflow progresses
- **Complete**: Show success state with checkmarks
- **Error**: Show where pipeline stopped + recovery options

## 🧪 Testing Plan
- [ ] Usability test: Can users explain the pipeline after seeing it?
- [ ] Accessibility audit: Screen reader announces stages
- [ ] Performance: Animation doesn't cause jank
- [ ] Cross-browser: Works in all supported browsers
- [ ] Reduced motion: Respects OS preference

## ⏱️ Estimated Effort
**3 days**

## 📝 Definition of Done
- [ ] Animation implemented
- [ ] Integrated with workflow state machine
- [ ] Tooltips added
- [ ] Accessibility tested
- [ ] Onboarding integration complete

## 📊 Progress
- [ ] Design mockups
- [ ] Animation implementation
- [ ] State machine integration
- [ ] Accessibility additions
- [ ] Testing
