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

Fabric/Power BI scopes are requested via **separate** integration grants (`integrations/microsoft/scopes.rs`). Power BI's consent flow is **built**; Fabric's is still **planned** (its scope list is a placeholder pending finalization against Microsoft's docs).

### Security — **built**

- Tokens never logged
- Tokens never sent to frontend (`AccountStatus` exposes email/name only)
- `AuthManager::encrypt_bytes` / `decrypt_bytes` for token blobs
- Sign-out clears local identity bundle (does not revoke at provider)

### Planned

- Multi-account support (schema allows; UI/store currently one active account)
- Tenant-aware consent UI
- Token refresh command surface for integration grants (a grant's access
  token is used until it expires; nothing refreshes it automatically)
- Provider-side revocation helper

## Layer B: Power BI grant — **built**

An `IntegrationGrant` distinct from the Layer A identity grant, requested only
when the user explicitly clicks "Connect Power BI" in Settings (gated behind
`--features experimental`) — signing in with Microsoft alone never grants it.

- **Requesting a grant**: `MicrosoftIntegrationService::request_power_bi_grant`
  requires an existing signed-in identity, then calls
  `identity::run_grant_flow` — the same PKCE/loopback/token-exchange core as
  `run_sign_in_flow` (factored out into a shared `authorize_and_exchange`
  helper in `identity/oauth/flow.rs`), but requesting the
  `power_bi::SCOPES` API scope instead of identity scopes, and skipping the
  userinfo fetch (the identity already exists). The resulting tokens are
  persisted via `IdentityStore::add_grant`, which appends to — rather than
  replaces — the existing bundle; a second `power_bi_request_grant` call
  supersedes the prior Power BI grant rather than accumulating a duplicate.
- **Using a grant**: `MicrosoftIntegrationService::power_bi_access_token`
  checks the grant is active (`power_bi_grant_active`) and decrypts its
  token via `IdentityStore::access_token_for_grant`.
- **Revoking**: `MicrosoftIntegrationService::revoke_power_bi_grant` →
  `IdentityStore::revoke_grant` sets `revoked_at` locally (does not contact
  Microsoft).
- **Commands**: `power_bi_grant_status`, `power_bi_request_grant`,
  `power_bi_revoke_grant` (`commands/integrations.rs`, gated behind
  `--features experimental`). See `docs/power-bi-integration.md` for the
  export preview/push commands that consume the resulting token.

## Configuration

See `docs/integrations-roadmap.md` for client ID setup (`integrations.microsoft_client_id`, `GHOST_MS_CLIENT_ID`).
