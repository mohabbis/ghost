# Ghost

[![Download](https://img.shields.io/badge/Download-Latest-8d7bff?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![macOS](https://img.shields.io/badge/macOS-12+-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10/11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Desktop file organization you approve before it acts.**

Ghost is a local-first desktop app for macOS and Windows that automates file organization with full preview, approval, and undo.

## The Problem

You have folders full of messy files: downloads, build artifacts, test reports, client documents, invoices. You need them organized, but you don't trust black-box automation to just... move things.

## The Solution: Ghost Organizer

```
Select folder -> Scan -> Preview plan -> Approve -> Execute -> Audit log -> Undo if needed
```

**Key features:**
- **Preview first**: See every proposed move/rename before anything changes
- **Approve required**: Nothing happens without your explicit approval
- **Undo available**: Every operation can be reversed
- **Local only**: No cloud, no data leaving your machine
- **Audit trail**: Full log of what changed and why

## 5-Minute Demo

1. Open Ghost
2. Select a folder (e.g., `~/Downloads`)
3. Click Scan - Ghost analyzes files without changing anything
4. Review the proposed plan in the UI
5. Click Approve - Ghost executes and logs everything
6. Use Undo anytime to revert

That's it. No AI agents, no cloud sync, no silent automation. Just safe, reviewable file organization.

## Download

Get the latest release for your platform:
- [macOS (Intel/Apple Silicon)](https://github.com/mohabbis/ghost/releases/latest)
- [Windows 10/11](https://github.com/mohabbis/ghost/releases/latest)

**Note for macOS users**: If you see a Gatekeeper warning, run:
```bash
xattr -dr com.apple.quarantine /Applications/Ghost.app
```

## Build from Source

### macOS
```bash
# Requires Xcode command line tools
cd apps/macos
swift build
```

### Windows/macOS (Tauri)
```bash
# Requires Node.js and Rust
npm install
npm run tauri build
```

## What Ghost Is Not

- ❌ Not a macro recorder
- ❌ Not a cloud agent
- ❌ Not an AI assistant that acts silently
- ❌ Not enterprise RPA

Ghost is a focused tool for one job: organizing files safely with human oversight.

## License

MIT License - see [LICENSE](LICENSE)
