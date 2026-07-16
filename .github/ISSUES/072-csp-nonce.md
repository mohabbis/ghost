---
issue_id: 072
parent_epic: 070
priority: P2
status: 🟡 Rescoped (2026-07-16)
labels: [security, frontend, csp]
---

# #072 Remove style-src 'unsafe-inline' via class migration (was: nonce-based CSP)

> **Rescope note (2026-07-16):** the nonce approach below is **infeasible in
> Tauri** — the CSP is a static string in `tauri.conf.json`, there is no
> per-request server to mint nonces, and Tauri's build-time hash injection does
> not cover runtime `innerHTML` templates. Current verified state:
>
> - `script-src` is already exactly `'self'` (no `unsafe-inline`) — the
>   security-relevant AC below is **met**, and it is now pinned by regression
>   tests in `src-tauri/tests/frontend_dom_contract.rs`
>   (`production_csp_stays_locked_down`, `index_html_has_no_inline_scripts`).
> - `object-src 'none'` and `frame-src 'none'` added (2026-07-16).
> - `style-src` still carries `'unsafe-inline'` because ~214 inline `style="…"`
>   attributes exist across `src/index.html` and `src/main.js` template
>   literals; style-src hashes do not apply to attributes.
>
> **Remaining scope:** migrate those inline style attributes to classes in
> `src/styles.css` so `'unsafe-inline'` can be dropped from `style-src`.
> Visual-QA-heavy refactor of a conflict-hotspot file; schedule deliberately.

## 📋 Summary
Remove `'unsafe-inline'` from Content Security Policy by implementing nonce-based script/style loading for production builds.

## 🎯 Why This Matters
- **Security**: `unsafe-inline` allows XSS attacks if HTML is compromised
- **Best practice**: Nonce-based CSP is industry standard for secure apps
- **Compliance**: Required for SOC 2 security controls
- **Trust**: Demonstrates serious security posture to enterprise users

## ✅ Acceptance Criteria
- [x] CSP header no longer contains `'unsafe-inline'` for script-src *(verified + pinned by test, 2026-07-16)*
- [ ] ~~Nonce generated per-request and injected into HTML~~ *(infeasible in Tauri — see rescope note)*
- [x] All inline scripts converted to external or use nonce attribute *(no inline scripts exist; pinned by `index_html_has_no_inline_scripts`)*
- [ ] Styles either externalized or use nonce (style-src) → **externalize the ~214 inline style attributes to classes**
- [x] Development mode still works (relaxed CSP acceptable for dev)
- [ ] No console errors in production build *(re-verify after style migration)*

## 🔗 Related Issues
- Parent Epic: #070 (Security: Audit, Harden, Document)
- Related: #032 (TypeScript migration), #037 (build experience)

## 🛠️ Implementation Notes
### Current State
Likely has CSP like:
```
Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; ...
```

### Target State
```
Content-Security-Policy: default-src 'self'; script-src 'self' 'nonce-{RANDOM}'; style-src 'self' 'nonce-{RANDOM}'; ...
```

### Implementation Approach

**Backend (Tauri/Rust):**
```rust
// Generate nonce per request
let nonce = generate_secure_nonce(); // 16+ bytes, base64 encoded

// Inject into HTML template
let html = template.replace("{CSP_NONCE}", &nonce);

// Set CSP header
let csp = format!(
    "default-src 'self'; script-src 'self' 'nonce-{}'; style-src 'self' 'nonce-{}'",
    nonce, nonce
);
```

**Frontend:**
```html
<!-- Before -->
<script>console.log('inline');</script>

<!-- After -->
<script nonce="{CSP_NONCE}">console.log('inline');</script>
```

Or better, move to external files:
```html
<script src="/assets/app.js" nonce="{CSP_NONCE}"></script>
```

### Tauri-Specific Considerations
- Tauri loads local files via `tauri://` protocol
- May need to adjust CSP for Tauri's IPC mechanisms
- Check Tauri docs for recommended CSP configuration

## 🧪 Testing Plan
- [ ] Run OWASP ZAP scan before and after
- [ ] Verify no CSP violations in browser console
- [ ] Test all interactive features still work
- [ ] Check development mode unaffected
- [ ] Validate with https://csp-evaluator.withgoogle.com/

## ⏱️ Estimated Effort
**1-2 days**

## 📝 Definition of Done
- [ ] CSP updated without `unsafe-inline`
- [ ] Nonce generation implemented
- [ ] All scripts/styles compatible
- [ ] Security scan passes
- [ ] Documentation updated

## 📊 Progress
- [ ] Audit current CSP usage
- [ ] Implement nonce generation
- [ ] Update templates
- [ ] Migrate inline scripts
- [ ] Testing
