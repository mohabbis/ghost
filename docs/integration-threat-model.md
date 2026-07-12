# Integration Threat Model

Status: **built** (documentation); controls **partially built**

## Guiding question

> Can a model, external client, business-system integration, compromised token, or malformed provider response cause Ghost to perform a mutation the user did not review and approve?

If yes, the design is wrong.

## Threats and mitigations

| Threat | Mitigation status |
|---|---|
| Malicious MCP client executes without approval | **planned** — approval tokens + MCP pairing |
| Prompt injection via filenames/metadata | **partially built** — `suggestion_is_safe`, redaction |
| Compromised local model endpoint | **planned** — localhost/LAN warnings, local-only routing |
| Stolen refresh token | **built** — encrypted at rest; narrow identity scopes; separate integration grants |
| Malicious Fabric workspace response | **planned** — schema validation, export preview, no inbound mutation |
| Stale / replayed approval tokens | **partially built** — claims + expiry helper; signing **planned** |
| Plan drift after approval | **planned** — plan hash revalidation at execution |
| Poisoned provider output (executable JSON) | **partially built** — suggestion schema + safety heuristics |
| Localhost service impersonation | **planned** — MCP pairing, explicit endpoint config |
| Excessive OAuth scopes | **built** — identity scopes separate from Fabric/PBI grants |
| Tenant confusion (wrong Microsoft tenant) | **partially built** — `tenant_id` on identity; UI **planned** |
| Cross-account data leakage | **planned** — grant scoped to `account_id` |
| Silent data export | **built** — no export commands yet; design requires preview + approval |
| Auto-approval by AI client | **built** — MCP tools cannot approve; documented denial |

## Layer separation

Do not merge:

- `identity/` — who signed in
- `integrations/` — Fabric/Power BI data plane
- `intelligence/` — suggestion-only internal models
- `mcp/` — external client tool surface

## Related

- `docs/threat-model.md` — product-wide threat model
- `docs/mcp-integration.md`
- `docs/approval-tokens.md`
