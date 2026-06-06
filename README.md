# Ghost 🦜 — Your AI Automation Parrot

**Vision**: AI assistant that pays attention → learns patterns → helps automate. Not a macro recorder.

Ghost watches what you do, captures semantic context (what you clicked, not just where), and eventually learns to automate repetitive tasks with intention.

## Stack

- **Tauri 2** (Rust + vanilla TypeScript, no bundler)
- **macOS-only** (uses CGEventTap + Accessibility APIs)

## Architecture

```
src-tauri/src/
├── commands.rs          # IPC: check_accessibility, start/stop_recording, replay_click
└── macos_ax.rs          # ★ CGEventTap + AX (dedicated run-loop thread)

src/
├── index.html           # Dark UI with brand colors
├── main.ts              # Listens ghost:click-captured, drives IPC
└── styles.css           # --accent:#7c5cff, --record:#ff4d5e
```

## Current Features

✅ **CGEventTap** fires on dedicated thread (run-loop bug fixed)  
✅ **Semantic Capture**: Captures element role, title, and description via AX APIs  
✅ **Human-readable UX**: Shows "Clicked 'Submit' button" instead of just coordinates  
✅ **Permission UX**: Record never blocked; banner hides on first captured click  
✅ **IPC**: Enriched `{x, y, element: {role, title, description}}` streams Rust→JS  
✅ **Replay**: Clicks replay at captured positions via enigo  

## Running

```bash
# Development
cargo tauri dev

# Build (test permissions with built binary)
cargo tauri build
```

⚠️ **macOS Gotchas**:
1. `AXIsProcessTrustedWithOptions` caches false → let clicks prove permission
2. `cargo tauri dev` binary path changes → test grants with `cargo tauri build`
3. CGEventTap needs running run loop (fixed ✅)

## Roadmap

### Priority #1: Semantic Capture ✅
- [x] Rust: AXUIElementCopyAttributeValue for role, title, description
- [x] JS: Render enriched payload ("Clicked 'Submit' button")
- [x] Store full action for pattern detection

### Next Steps
1. **Pattern Detection**: Cluster sequences, detect repetition
2. **On-device LLM**: Generate automation suggestions
3. **Natural Language**: "I noticed you X every time — automate it?"

## Brand

Friendly, helpful, "like a parrot" — learns by repeating with intention.

---

*Built with ❤️ for macOS automation*
