# Ghost Deployment Guide

## 🏗️ Project Structure

```
ghost/
├── src/                    # Tauri app frontend (desktop application)
│   ├── index.html         # App UI (identical to public/index.html)
│   ├── main.js            # App JavaScript
│   └── styles.css         # App styles
│
├── public/                 # Marketing website (ghost.muharafiq.com)
│   ├── index.html         # Website UI (identical to src/index.html)
│   ├── main.js            # Website JavaScript
│   ├── styles.css         # Website styles
│   └── downloads/         # Placeholder (actual downloads from GitHub Releases)
│
├── src-tauri/             # Rust backend for desktop app
│   ├── src/               # Rust source code
│   ├── Cargo.toml         # Rust dependencies
│   └── tauri.conf.json    # Tauri configuration
│
└── .github/workflows/     # CI/CD pipelines
    ├── rust.yml           # Continuous integration (build, test, lint)
    ├── release.yml        # Release builds for macOS and Windows
    └── deploy-website.yml # (Not used - website hosted externally)
```

## 🎯 Deployment Strategy

### 1. Desktop Application

**Platform:** macOS and Windows  
**Distribution:** GitHub Releases  
**Workflow:** `.github/workflows/release.yml`

#### Release Process

1. **Tag a release:**
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

2. **Automated build:**
   - macOS: Universal binary (Apple Silicon + Intel) → `Ghost.dmg`
   - Windows: NSIS installer → `Ghost_Setup.exe`

3. **GitHub Release:**
   - Binaries automatically uploaded to GitHub Releases
   - Release notes auto-generated from commits

#### Download Links

- **macOS:** `https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg`
- **Windows:** `https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe`

### 2. Marketing Website

**Domain:** [ghost.muharafiq.com](https://ghost.muharafiq.com)  
**Source:** `public/` directory  
**Hosting:** External (custom domain)

#### Website Deployment

The marketing website is hosted externally at ghost.muharafiq.com. To update:

1. **Make changes in `public/` directory**
2. **Sync changes to hosting provider** (manual or via custom deployment)
3. **Verify at:** https://ghost.muharafiq.com

**Note:** The `src/` directory contains identical files for the Tauri app. Keep both in sync when making UI changes.

### 3. Continuous Integration

**Workflow:** `.github/workflows/rust.yml`  
**Triggers:** Push to main/master/develop, Pull Requests

#### CI Pipeline

- ✅ **Check:** Verify compilation
- ✅ **Test:** Run test suite
- ✅ **Clippy:** Lint with warnings as errors
- ✅ **Format:** Check code formatting
- ✅ **Build Matrix:** Test on Ubuntu, macOS, Windows

## 🔧 Development Workflow

### Local Development

```bash
# Install dependencies
cargo install tauri-cli --version "^2.0"

# Run in development mode
cargo tauri dev

# Build for production
cargo tauri build
```

### Making Changes

#### Desktop App UI Changes

1. Edit files in `src/` directory
2. Test with `cargo tauri dev`
3. Also update `public/` to keep website in sync

#### Website Changes

1. Edit files in `public/` directory
2. Test locally (open `public/index.html` in browser)
3. Also update `src/` to keep desktop app in sync
4. Deploy to ghost.muharafiq.com

#### Backend Changes

1. Edit Rust files in `src-tauri/src/`
2. Test with `cargo check` and `cargo test`
3. Run `cargo clippy` to catch issues
4. Format with `cargo fmt`

## 📦 Release Checklist

- [ ] Update version in `src-tauri/Cargo.toml`
- [ ] Update version in `src-tauri/tauri.conf.json`
- [ ] Update `CHANGELOG.md` (if exists)
- [ ] Commit changes: `git commit -m "chore: bump version to X.Y.Z"`
- [ ] Create and push tag: `git tag vX.Y.Z && git push origin vX.Y.Z`
- [ ] Wait for GitHub Actions to build and release
- [ ] Verify downloads work from GitHub Releases
- [ ] Update website if needed (sync `public/` to hosting)
- [ ] Test downloads on both macOS and Windows

## 🔍 Troubleshooting

### CI Failures

**Rust compilation errors:**
```bash
cd src-tauri
cargo check
cargo clippy
```

**Test failures:**
```bash
cd src-tauri
cargo test
```

**Format issues:**
```bash
cd src-tauri
cargo fmt
```

### Release Build Issues

**macOS signing:**
- Ad-hoc signed by default (users need to clear quarantine)
- For notarized builds, add Apple Developer ID secrets to GitHub

**Windows installer:**
- NSIS installer created by default
- MSI available as fallback

### Website Issues

**Download links not working:**
- Verify GitHub Release exists
- Check download URLs point to `/releases/latest/download/`

**Website out of sync with app:**
- Compare `src/index.html` with `public/index.html`
- Keep both directories synchronized

## 🚀 Quick Commands

```bash
# Development
cargo tauri dev                    # Run app in dev mode
cargo check --manifest-path src-tauri/Cargo.toml  # Quick check
cargo clippy --manifest-path src-tauri/Cargo.toml # Lint

# Testing
cargo test --manifest-path src-tauri/Cargo.toml   # Run tests

# Building
cargo tauri build                  # Production build
cargo tauri build --target universal-apple-darwin  # macOS universal

# Release
git tag v1.0.0 && git push origin v1.0.0  # Trigger release workflow
```

## 📚 Additional Resources

- [Tauri Documentation](https://tauri.app)
- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Rust Book](https://doc.rust-lang.org/book/)
- [Project README](README.md)