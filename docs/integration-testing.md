# Integration Testing

Status: **partially built**

## OAuth / identity — **built**

| Test | Location |
|---|---|
| PKCE challenge = SHA256(verifier) | `identity/oauth/pkce.rs` |
| State mismatch rejection | `identity/oauth/flow.rs` (runtime) |
| Provider parse / client ID config | `identity/oauth/provider.rs` |
| Token not in identity JSON plaintext | `identity/store.rs` |
| Legacy account.json migration | `identity/store.rs` |
| Identity without Fabric grant denied | `integrations/microsoft/mod.rs` |
| Account command round-trip | `commands/account.rs`, `accounts.rs` |

## Provider — **partially built**

| Test | Status |
|---|---|
| Safe suggestion validation | **built** — `intelligence/schema.rs` |
| Executable field rejection | **built** — `intelligence/parse.rs` |
| JSON fence parsing | **built** — `intelligence/parse.rs` |
| Disabled provider default | **built** — `intelligence/router.rs` |
| Confidential payload blocked | **built** — `intelligence/router.rs` |
| Encrypted API key storage | **built** — `intelligence/credentials.rs` |
| OpenAI/Anthropic live API | manual — `intelligence_test_provider` command |
| Schema fuzzing | **planned** |

## MCP — **partially built**

| Test | Status |
|---|---|
| Execute requires approval flag | **built** — `mcp/tools.rs` |
| Expired approval claims | **built** — `mcp/approval.rs` |
| No execute without token | **planned** |
| Plan drift invalidates token | **planned** |

## Fabric / Power BI — **partially built**

| Test | Status |
|---|---|
| Identity ≠ integration grant | **built** |
| Export preview = payload | **planned** |
| Sensitive fields excluded | **planned** |
| No inbound mutation trigger | **planned** (design + future tests) |

## Regression — **built**

Organizer/policy/audit/undo tests remain in existing modules; run full suite:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Experimental features: add `--features experimental` when touching gated code.
