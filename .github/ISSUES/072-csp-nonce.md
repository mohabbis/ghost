---
issue_id: 072
parent_epic: 070
priority: P1
status: ⚪ Todo
labels: [security, frontend, csp]
---

# #072 Replace 'unsafe-inline' CSP with Nonce-Based Approach

## 📋 Summary
Remove `'unsafe-inline'` from Content Security Policy by implementing nonce-based script/style loading for production builds.

## 🎯 Why This Matters
- **Security**: `unsafe-inline` allows XSS attacks if HTML is compromised
- **Best practice**: Nonce-based CSP is industry standard for secure apps
- **Compliance**: Required for SOC 2 security controls
- **Trust**: Demonstrates serious security posture to enterprise users

## ✅ Acceptance Criteria
- [ ] CSP header no longer contains `'unsafe-inline'` for script-src
- [ ] Nonce generated per-request and injected into HTML
- [ ] All inline scripts converted to external or use nonce attribute
- [ ] Styles either externalized or use nonce (style-src)
- [ ] Development mode still works (relaxed CSP acceptable for dev)
- [ ] No console errors in production build

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
