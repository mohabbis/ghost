# Ghost Demo Script: Claude + AI Agent Safety

## 5-Minute Video Script for YC Application

### Scene 1: The Problem (0:00-0:30)
**Visual**: Screen recording of Claude Desktop attempting to process invoices

**Narration**: 
"Meet Sarah, a bookkeeper who uses Claude to help process invoices. She asks Claude: 'Move all paid invoices from Downloads to Archive.'"

**Visual**: Claude executes the command... but accidentally moves an UNPAID invoice that had "paid" in the filename.

**Narration**: 
"One hallucination. One misplaced file. Now she can't find an unpaid invoice and misses a payment. This is why enterprises can't trust AI agents with sensitive operations."

---

### Scene 2: Enter Ghost (0:30-1:30)
**Visual**: Ghost app icon appears, clean UI opens

**Narration**: 
"Ghost sits between AI agents and your sensitive operations. It's the approval, audit, and undo layer that makes autonomous agents safe."

**Visual**: Show MCP configuration in claude_desktop_config.json

**Narration**: 
"Install Ghost as an MCP server in Claude Desktop. Now every file operation goes through Ghost first."

---

### Scene 3: Live Demo - Policy Enforcement (1:30-3:00)
**Visual**: Sarah asks Claude: "Process all unpaid invoices in ~/Downloads"

**Narration**: 
"Sarah asks Claude to process invoices. But this time, Ghost intercepts the request."

**Visual**: Ghost UI pops up showing:
- 🔴 **Policy Alert**: "Batch file operations require approval"
- Preview: 12 files Claude wants to move
- Risk assessment: "3 files match pattern: potentially unpaid"

**Narration**: 
"Ghost checks the policy: batch operations need approval. It shows Sarah exactly what Claude plans to do. She reviews each file..."

**Visual**: Sarah clicks approve on 11 files, rejects 1 flagged file

**Narration**: 
"She approves 11 files, but Ghost flagged one - it was actually a quote, not an invoice. Without Ghost, Claude would have archived it. Now she catches the mistake."

---

### Scene 4: Audit & Compliance (3:00-3:45)
**Visual**: Ghost Audit Log view

**Narration**: 
"Every decision is logged. Sarah's compliance team can see: which files were moved, who approved, when, and why. This is SOC2-ready auditing out of the box."

**Visual**: Show audit log entries with timestamps, user IDs, file paths

**Narration**: 
"Six months later, an auditor asks: 'Who approved moving invoice #4521?' Sarah pulls up the Ghost audit log in seconds. No manual tracking needed."

---

### Scene 5: Undo Safety Net (3:45-4:30)
**Visual**: Sarah realizes she made a mistake

**Narration**: 
"Even with approvals, mistakes happen. Sarah accidentally approved a file she shouldn't have."

**Visual**: Sarah clicks "Undo" button in Ghost UI

**Narration**: 
"Ghost's undo system rolls back any action within 30 days. The file returns to its original location. No panic, no data loss."

---

### Scene 6: The Vision (4:30-5:00)
**Visual**: Montage of different use cases - engineer using Cursor, ops team managing configs, healthcare admin processing patient records

**Narration**: 
"Ghost isn't just for bookkeepers. Engineers using Cursor. Ops teams managing production configs. Healthcare admins processing patient records. Anyone running AI agents needs a trust layer."

**Visual**: Ghost logo with tagline

**Narration**: 
"In 5 years, no enterprise will run AI agents without a Ghost-like approval layer. We're building that future today."

**Text on screen**: 
- 🎯 Weekly Active Workflows: 10+ per team
- ⚡ Time Saved: 15 min per workflow
- 🛡️ Blocked Risks: Real hallucinations caught
- ↩️ Undo Rate: <5%

**Call to Action**: 
"Join our early access: founders@ghost.dev | YC W25 Application"

---

## Production Notes

### Required Assets
1. **Screen recordings**:
   - Claude Desktop asking questions
   - Ghost UI showing approvals
   - Audit log view
   - Undo action

2. **Graphics**:
   - Ghost app icon animation
   - MCP protocol diagram (AI → Ghost → Execute)
   - Metrics dashboard mockup

3. **Voiceover**:
   - Professional narrator or founder voice
   - Background music: subtle, tech-focused

### Tools Needed
- OBS Studio or ScreenFlow for recording
- Descript or Premiere Pro for editing
- Figma for graphics

### Distribution Channels
- YouTube (main video)
- Twitter/X (30-second clips)
- LinkedIn (enterprise angle)
- Hacker News launch post
- YC application video link

---

## Follow-Up Content Ideas

1. **"Claude Tried to Delete Production. Ghost Stopped It."** - Dramatic real-world example
2. **"How Bookkeepers Save 10 Hours/Week with Ghost"** - Customer testimonial format
3. **"MCP Protocol Explained: Connect Any AI to Ghost"** - Technical deep dive
4. **"SOC2 Compliance for SMBs: $29/month vs $50k/year"** - Enterprise sales angle
