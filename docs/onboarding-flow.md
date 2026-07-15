# First-Run Onboarding Flow

## Goal

Guide new users through the trust model in <2 minutes, so they understand:
1. Ghost is not an AI agent
2. Ghost requires approval before actions
3. Ghost shows what will happen before it happens
4. Ghost can undo changes

## Screen 1: Welcome

**Title**: "Welcome to Ghost"

**Subtitle**: "The trust layer for desktop automation"

**Body**:
```
Ghost helps you automate repetitive file work safely.

Before Ghost changes anything, you review exactly what will happen.
You approve it. Ghost executes it. You can undo it.

Ready to organize your first folder?
```

**Buttons**:
- "Get Started" → Screen 2
- "Learn More" → docs (external)
- "Skip" → Close onboarding

---

## Screen 2: The Trust Model (Visual)

**Title**: "How Ghost Works"

**Visual Flow** (animated, 3 steps):

```
Step 1: You select a folder
    ↓
    Icon: folder + hand pointing

Step 2: Ghost proposes changes
    Icon: documents + checkmarks

Step 3: You approve & Ghost executes
    Icon: play button + checkmark

Step 4: You can undo anytime
    Icon: undo arrow
```

**Caption**: "Ghost never acts without your approval."

**Buttons**:
- "Next" → Screen 3

---

## Screen 3: Pick Your First Folder

**Title**: "Choose a Folder to Organize"

**Subtitle**: "Ghost will only access this folder."

**Options** (radio buttons):

- [ ] Downloads
- [ ] Desktop
- [ ] Custom folder → file picker
- [ ] Skip for now

**Body**:
```
Ghost works with files in a single folder.
Choose one and Ghost will show you a preview before doing anything.
```

**Buttons**:
- "Next" (enabled only if folder selected)
- "Skip" → Screen 5

---

## Screen 4: Preview the First Run

**Title**: "Here's What Ghost Will Do"

**Layout**:

```
┌─────────────────────────────────────────┐
│ Before                                  │
├─────────────────────────────────────────┤
│ 📁 Downloads/                           │
│  ├─ invoice.pdf                         │
│  ├─ receipt.pdf                         │
│  ├─ statement.pdf                       │
│  └─ screenshot.png                      │
│                                         │
│ After                                   │
├─────────────────────────────────────────┤
│ 📁 Downloads/                           │
│  ├─ 📁 Invoices/                        │
│  │   └─ invoice.pdf                     │
│  ├─ 📁 Receipts/                        │
│  │   └─ receipt.pdf                     │
│  ├─ statement.pdf (unknown, will skip)  │
│  └─ screenshot.png (unknown, will skip) │
└─────────────────────────────────────────┘

Summary:
✓ 1 file moved (invoice)
✓ 1 file moved (receipt)
⊘ 2 files skipped (confidence too low)
```

**Key Message**:
```
✓ Ghost moves files into organized folders
✓ Ambiguous files are skipped (not deleted)
✓ You approve before anything happens
```

**Buttons**:
- "Approve & Execute" → execution
- "Edit" → edit destinations (advanced)
- "Cancel" → Screen 3

---

## Screen 5: Execution Progress

**Title**: "Organizing Your Folder"

**Layout**:

```
Progress bar: [████████░░] 80%

Current: Moving invoice.pdf to Invoices/

Completed: 1/2
Time remaining: ~2 seconds

[Cancel] (always available)
```

**Key Message**: "You can cancel anytime. Any changes made so far can be undone."

---

## Screen 6: Success!

**Title**: "Your Folder is Organized"

**Summary**:

```
✓ 1 file moved
✓ 1 folder created
✓ 2 files skipped
✓ 0 errors
```

**What Happened**:
- Downloads/invoice.pdf → Downloads/Invoices/invoice.pdf
- Downloads/receipt.pdf → Downloads/Receipts/receipt.pdf

**Buttons**:
- "View Audit Log" → audit details
- "Undo" (prominent) → reverses entire run
- "Done" → main UI
- "Organize Again" → Screen 3

---

## Screen 7: Undo (if user clicks Undo)

**Title**: "Undo That Run?"

**Message**:
```
This will move your files back to their original locations.

✓ invoice.pdf → Downloads/
✓ receipt.pdf → Downloads/
✓ Invoices/ folder → remove (will be empty)
✓ Receipts/ folder → remove (will be empty)
```

**Buttons**:
- "Undo" → reverses
- "Cancel" → stay on success screen

**After Undo**:
```
Title: "Undone"
Message: "Your folder is back to how it was. You can organize it again anytime."
[Done]
```

---

## Screen 8: Settings & Zones (Bonus)

**Title**: "Manage Your Zones"

**Message**:
```
Zones are folders where Ghost is allowed to work.
You control exactly which folders Ghost can touch.
```

**List**:

```
✓ Downloads (active)
✓ Desktop (active)

[Add Zone] [Remove]
```

**Buttons**:
- "Done" → main UI

---

## Onboarding Flow Chart

```
Welcome
  ↓
How Ghost Works
  ↓
Pick Folder (or Skip)
  ├─ Skip → Settings (optional)
  │   ↓
  │ [Done]
  │
  └─ Folder Selected
     ↓
     Preview Plan
     ├─ Cancel → Pick Folder
     ├─ Edit → advanced options
     └─ Approve
        ↓
        Execute
        ↓
        Success
        ├─ Undo → Undone → Done
        ├─ View Audit → audit details → Done
        └─ Done → Main UI
```

---

## Timing

- **Total time**: 90 seconds (without skipping)
- Screen 1 (Welcome): 15 seconds
- Screen 2 (How it works): 20 seconds
- Screen 3 (Pick folder): 20 seconds
- Screen 4 (Preview): 20 seconds
- Screen 5 (Execute): 10 seconds
- Screen 6 (Success): 15 seconds

---

## UX Principles

### 1. Build Trust Through Transparency
Every screen shows exactly what will happen.
No hidden behavior. No "magic."

### 2. User Always Approves
Before execution, user reviews and approves.
No auto-actions, ever.

### 3. Emphasize Reversibility
Undo is prominent and easy.
User can always reverse what Ghost did.

### 4. Admit Limitations
Files skipped because "confidence too low"?
Say that plainly. Don't hide it.

### 5. Respect User Time
Total flow is ~2 minutes.
No unnecessary screens.
Skip button available throughout.

---

## Accessibility

- [ ] Keyboard navigation (Tab, Enter, Escape)
- [ ] Screen reader support (ARIA labels)
- [ ] High contrast mode
- [ ] Large text support (zoom)
- [ ] Color not the only differentiator (icons + text)

---

## Testing Checklist

- [ ] First-time user completes flow in <2 minutes
- [ ] User understands they must approve before action
- [ ] User sees preview before execution
- [ ] User can undo after execution
- [ ] User can skip onboarding
- [ ] Keyboard navigation works
- [ ] Screen reader reads all content
- [ ] Mobile-friendly (if applicable)

---

## Implementation Notes

### Technical

- Built in Tauri (vanilla JS + HTML/CSS, no framework)
- State management: simple JS object with current screen
- Animations: CSS transitions (no heavy JS)
- No external dependencies (performance first)
- Responsive: desktop primary, mobile secondary

### File Structure

```
src/
├── onboarding.js          # Flow controller + screens
├── onboarding.css         # Styling
├── onboarding.html        # Markup
└── onboarding-data.json   # Screen content (for i18n later)
```

### Integration Points

- Tauri command: `select_folder()`
- Tauri command: `organizer_plan()`
- Tauri command: `organizer_execute()`
- Tauri command: `get_audit_log()`
- Tauri command: `organizer_undo()`

### Future Enhancements

- Internationalization (i18n)
- Analytics on onboarding completion (opt-in)
- A/B testing different flows
- Customizable presets (invoice, receipt, etc.)

---

**Version**: 1.0  
**Status**: Ready for Phase 4 implementation
