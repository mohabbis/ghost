# Organizer Polish: Error Messages & UX Improvements

## Error Messages

Good error messages **explain what happened, why it happened, and what the user can do about it**.

### Pattern: Specific > Vague

| Bad | Better |
|-----|--------|
| "Something went wrong" | "Ghost could not access the Downloads folder. Permission denied on /Users/alice/Downloads/.ghost." |
| "Replay failed" | "Step 3 failed: The 'Save' button was not found. This might mean the app changed or the window moved. You can: (1) Retry this step, (2) Resume from here, or (3) Undo all changes." |
| "Error" | "Conflict: A file named 'invoice.pdf' already exists in Invoices/2026/. Choose: (1) Rename to 'invoice_2026-07-06_1.pdf', (2) Skip this file, or (3) Replace the existing file." |

### Error Categories

#### Permission Errors

```
Title: "Permission Denied"

Message:
"Ghost cannot access the Downloads folder.
Permission: Your computer has restricted access.

What you can do:
• Grant permission: Settings → Privacy & Security → Ghost
• Choose a different folder
• Contact support if this persists"

Buttons:
- [Grant Permission] (opens system settings)
- [Choose Another Folder]
- [Learn More]
```

#### Conflict Errors

```
Title: "File Conflict"

Message:
"A file named 'invoice.pdf' already exists in Invoices/2026/.

What should Ghost do?

◯ Rename the file to 'invoice_2026-07-06_1.pdf'
◯ Skip this file (leave original alone)
◯ Replace the existing file (careful: this overwrites)"

Buttons:
- [Apply] → executes chosen action
- [Cancel] → return to preview
```

#### Zone Boundary Errors

```
Title: "Operation Outside Allowed Folder"

Message:
"Ghost tried to move a file outside the Downloads folder.
This is blocked for safety.

Reason: Ghost can only work inside folders you explicitly allow.

What you can do:
• Add a new Zone: Settings → Zones → [Add Zone]
• Remove this file from the plan and try again
• Review the plan: [Show Plan]"

Buttons:
- [Add Zone]
- [Remove File from Plan]
- [Show Plan]
```

#### Policy Block Errors

```
Title: "Action Blocked by Policy"

Message:
"Ghost Guard blocked: Delete file 'old_invoice.pdf'

Why: Delete operations are blocked by default to prevent data loss.

What you can do:
• Enable delete in Settings → Policy → [Allow delete]
• Skip this file and leave it in place
• Clear it manually outside Ghost"

Buttons:
- [Enable Delete]
- [Skip This File]
- [Done]
```

#### Partial Execution Errors

```
Title: "Execution Stopped at Step 7 of 50"

Message:
"An error occurred while moving 'receipt_001.pdf':
Permission denied on destination folder.

Status:
• Completed: 6 files successfully moved
• Failed at: Step 7 of 50
• Remaining: 43 files not yet processed

What you can do:
• [Resume from Step 7] → continue where it stopped
• [Undo Completed Steps] → reverse what was done
• [Cancel] → stop here"

Buttons:
- [Resume from Step 7]
- [Undo Completed]
- [Cancel]
```

#### Undo Journal Corruption Error

```
Title: "Undo Journal Invalid"

Message:
"Ghost cannot undo this run. The undo data is corrupted or missing.

Reason: This might happen if:
• The filesystem was damaged
• Files were moved/deleted outside Ghost
• There's a software bug (please report)

What you can do:
• Manually restore files to their original locations
• Report this issue to Ghost support
• Check the audit log to see what changed"

Buttons:
- [View Audit Log]
- [Report Issue]
- [Done]
```

---

## Keyboard Shortcuts

### Global Shortcuts

| Shortcut | Action | Context |
|----------|--------|---------|
| `Cmd+,` / `Ctrl+,` | Open Settings | Anywhere |
| `Cmd+/` / `Ctrl+/` | Help (shortcut cheat sheet) | Anywhere |
| `Escape` | Close dialog / Cancel operation | In modal |

### Organizer Shortcuts

| Shortcut | Action | Context |
|----------|--------|---------|
| `Cmd+Shift+O` / `Ctrl+Shift+O` | Open Organizer | Main window |
| `Cmd+F` / `Ctrl+F` | Filter files in preview | Preview screen |
| `Enter` | Approve plan | Preview screen |
| `Escape` | Cancel and return to folder select | Preview screen |
| `↑` / `↓` | Select previous/next file | Preview screen (table) |
| `Space` | Toggle file selection | Preview screen (table) |
| `Cmd+A` / `Ctrl+A` | Select all files | Preview screen (table) |
| `Cmd+D` / `Ctrl+D` | Deselect all files | Preview screen (table) |
| `←` / `→` | Change sort column | Preview screen (table) |
| `R` | Rename mode for selected file | Preview screen (table) |
| `E` | Edit destination for selected file | Preview screen (table) |

### Success Screen Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+Z` / `Ctrl+Z` | Undo |
| `Cmd+E` / `Ctrl+E` | Export audit log |
| `Cmd+Shift+L` / `Ctrl+Shift+L` | View audit log |
| `Enter` / `Space` | Approve and re-organize | (if organizing again) |

### Audit Log Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+F` / `Ctrl+F` | Search operations |
| `Cmd+S` / `Ctrl+S` | Export as CSV/JSON |
| `Cmd+P` / `Ctrl+P` | Print or save to PDF |

---

## Shortcut Discovery

### Option 1: Keyboard Help Modal

Press `Cmd+/` or `Ctrl+/` to open:

```
╔═══════════════════════════════════════╗
║         Keyboard Shortcuts            ║
╠═══════════════════════════════════════╣
║                                       ║
║ Organizer                             ║
║ ─────────────────────────────────     ║
║ Cmd+Shift+O    Open Organizer         ║
║ Enter          Approve & Execute      ║
║ Escape         Cancel                 ║
║ Cmd+F          Search Files           ║
║                                       ║
║ Success Screen                        ║
║ ─────────────────────────────────     ║
║ Cmd+Z          Undo                   ║
║ Cmd+E          Export Audit           ║
║                                       ║
║ Global                                ║
║ ─────────────────────────────────     ║
║ Cmd+,          Settings               ║
║ Cmd+/          This Help              ║
║                                       ║
╚═══════════════════════════════════════╝

[Close]
```

### Option 2: Inline Hints

Preview table columns show hints:

```
File               ⌘F to filter
Confidence         ↑↓ to sort
Destination        E to edit
```

Buttons show keyboard equivalent:

```
[Approve & Execute]  [Enter]
[Cancel]             [Esc]
```

### Option 3: Tooltips

Hover over buttons:

```
Approve & Execute
This will move 43 files. You can undo anytime.
Keyboard: Enter or Cmd+Enter
```

---

## Polish Details

### Focus States

Keyboard navigation requires clear focus indicators:

```css
/* Button focus */
button:focus {
  outline: 2px solid #8d7bff;  /* Ghost purple */
  outline-offset: 2px;
}

/* Table row focus */
tbody tr:focus {
  background-color: rgba(141, 123, 255, 0.1);
  box-shadow: inset 0 0 0 2px #8d7bff;
}
```

### Dark Mode

Support system preference:

```css
@media (prefers-color-scheme: dark) {
  body {
    background-color: #1a1a1a;
    color: #ffffff;
  }
  .error-message {
    background-color: #2a2a2a;
    border-color: #8d7bff;
  }
}
```

Optional toggle in Settings:

```
Settings → Appearance
◯ Light
◯ Dark
◉ System (follow OS preference)
```

### Animations

Subtle, fast animations build polish:

```css
/* Modal slide-up */
.modal {
  animation: slideUp 0.2s ease-out;
}

@keyframes slideUp {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* Progress bar fill */
.progress-bar {
  transition: width 0.1s linear;
}

/* Success check mark */
.checkmark {
  animation: checkmark 0.6s ease-out;
}
```

### Responsiveness

Desktop primary, mobile secondary:

```css
/* Desktop (primary) */
.preview-table { display: table; }
.keyboard-hint { display: inline; }

/* Mobile (medium screens) */
@media (max-width: 1024px) {
  .preview-table { display: block; }  /* Cards layout */
  .keyboard-hint { display: none; }   /* Touch instead */
}

/* Tablet/Phone */
@media (max-width: 600px) {
  .preview-table { display: block; }
  button { font-size: 18px; }  /* Touch-friendly */
  padding { increase; }
}
```

---

## Testing Checklist

- [ ] All errors are actionable (suggest a fix)
- [ ] Keyboard shortcuts work as documented
- [ ] Tab navigation is logical and continuous
- [ ] Focus indicators are always visible
- [ ] Dark mode works and looks good
- [ ] Mobile responsiveness tested (if applicable)
- [ ] Animations are smooth and under 300ms
- [ ] All buttons have hover states
- [ ] All form inputs show validation state
- [ ] Error messages are not too technical
- [ ] Success messages celebrate without condescension

---

## Before/After Examples

### Before: Generic Error

```
Error: PERMISSION_DENIED
```

### After: Specific & Actionable

```
Ghost couldn't move 'invoice.pdf' to Invoices/2026/.

Reason: Permission denied on the destination folder.

What you can do:
• Check that you have permission to create folders in Invoices/
• Try a different destination folder
• Contact your system administrator

Details: /Users/alice/Documents/Invoices/2026/
Error code: EACCES
```

---

**Version**: 1.0  
**Status**: Ready for Phase 4.2 implementation
