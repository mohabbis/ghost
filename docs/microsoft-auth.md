# Microsoft Account and OAuth Architecture

Status key: **built** | **partially built** | **planned**

## Layer A: identity only

Microsoft/Google sign-in answers **who** the user is. It does **not** grant Fabric, Power BI, or Google Cloud access.

### Built

- OAuth 2.0 Authorization Code + PKCE (public client, no embedded secret)
- Loopback redirect on `127.0.0.1` with runtime port
- State validation and 180s callback timeout
- System browser consent (`open` crate)
- Refresh token storage (encrypted when vault password configured)
- `AccountIdentity` separated from `IntegrationGrant` and `TokenRecord`
- Legacy `account.json` migration to `identity.json` on first load
- Commands: `account_status`, `account_sign_in`, `account_sign_out` (unchanged IPC)

### Module layout

```text
src-tauri/src/identity/
├── types.rs          AccountIdentity, IntegrationGrant, TokenRecord
├── store.rs          IdentityStore (encrypted bundle)
├── errors.rs         IntegrationError
└── oauth/
    ├── pkce.rs
    ├── provider.rs   OAuthProvider (identity scopes only)
    ├── callback.rs
    └── flow.rs       interactive sign-in
```

Local vault password encryption remains in `auth.rs` (separate concern).

### Data model — **built**

```rust
AccountIdentity   // metadata only — no tokens, no integration scopes
IntegrationGrant    // per-integration consent (Identity, Fabric, Power BI, …)
TokenRecord         // encrypted access/refresh material keyed by grant_id
```

### Identity sign-in scopes — **built**

| Provider | Scopes |
|---|---|
| Microsoft | `openid email profile offline_access` |
| Google | `openid email profile` |

Fabric/Power BI scopes are requested via **separate** integration grants (`integrations/microsoft/scopes.rs`) — **planned** consent flow.

### Security — **built**

- Tokens never logged
- Tokens never sent to frontend (`AccountStatus` exposes email/name only)
- `AuthManager::encrypt_bytes` / `decrypt_bytes` for token blobs
- Sign-out clears local identity bundle (does not revoke at provider)

### Planned

- Multi-account support (schema allows; UI/store currently one active account)
- Tenant-aware consent UI
- Token refresh command surface for integration grants
- Provider-side revocation helper

## Configuration

See `docs/integrations-roadmap.md` for client ID setup (`integrations.microsoft_client_id`, `GHOST_MS_CLIENT_ID`).
