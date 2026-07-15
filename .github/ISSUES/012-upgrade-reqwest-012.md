---
issue_id: 012
parent_epic: 010
priority: P1
status: ⚪ Todo
labels: [rust, networking, backend]
---

# #012 Replace reqwest 0.11 → 0.12 (hyper 1.x)

## 📋 Summary
Upgrade `reqwest` HTTP client from 0.11 to 0.12 to leverage hyper 1.x for better async performance and reduced dependencies.

## 🎯 Why This Matters
- **Performance**: hyper 1.x has improved async I/O handling for concurrent requests
- **Dependencies**: Removes deprecated crates, reduces attack surface
- **Future-proofing**: Aligns with Rust ecosystem direction
- **Organizer scans**: Faster network calls when fetching remote workflow definitions

## ✅ Acceptance Criteria
- [ ] `reqwest = "0.12"` in `Cargo.toml`
- [ ] All HTTP client code updated for API changes
- [ ] Connection pooling still works correctly
- [ ] Timeout configurations preserved
- [ ] Error handling maintains same user-facing messages
- [ ] Benchmarks show no regression (or improvement)

## 🔗 Related Issues
- Parent Epic: #010 (Rust Backend: Stability + Performance)
- Related: #011 (Tauri upgrade), #013 (tokio tuning)

## 🛠️ Implementation Notes
Key breaking changes in reqwest 0.12:
- `ClientBuilder` API tweaks
- TLS backend selection changes (rustls vs native-tls)
- Redirect policy configuration
- Body type conversions

Check:
```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
```

## 🧪 Testing Plan
- [ ] Manual: Test Organizer scan with remote URLs
- [ ] Manual: Verify workflow download still works
- [ ] Automated: Run integration tests with mock HTTP server
- [ ] Edge case: Test timeout behavior on slow connections

## ⏱️ Estimated Effort
**1 day**

## 📝 Definition of Done
- [ ] Code complete
- [ ] Tests passing
- [ ] Benchmarks run
- [ ] Documentation updated

## 📊 Progress
- [ ] Research breaking changes
- [ ] Update dependencies
- [ ] Fix compilation errors
- [ ] Test thoroughly
- [ ] Benchmark comparison
