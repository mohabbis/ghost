# Threat model (Ghost Cloud)

Scope: the SaaS in `cloud/` (web, worker, Postgres, Redis, Playwright, future connectors).

## Assets

- Customer workflow definitions and run history
- Connector credentials (future)
- Audit logs (integrity matters as much as secrecy)
- Browser session state during a run

## Trust boundaries

| Boundary | Assumption |
|---|---|
| Browser → Next.js API | Session JWT; org-scoped queries |
| Web → Redis → Worker | Jobs carry `runId` + `orgId`; worker re-loads from DB |
| Worker → target sites | Ephemeral browser; no standing credentials in Phase 1 |
| Operator → Approve UI | Only org members resolve approvals |

## Top threats & controls

| Threat | Control |
|---|---|
| Cross-tenant data leak | Every query filtered by `orgId` from session |
| Sensitive action without human | `classifyStep` + approval gate in state machine |
| Prompt/injection driving mutations | AI never executes; only deterministic runner |
| Credential theft (future connectors) | Envelope encryption, least privilege, revoke |
| Audit tampering | Hash-chained events; verify endpoint (planned) |
| Run continues after cancel | Worker checks `CANCELED` each step |
| Screenshot leakage of secrets | Redact; never capture password fields into artifacts |

## Out of scope (for now)

- Nation-state attackers
- Physical access to Postgres disks (use managed DB encryption)
- Legacy desktop threat model (see code + historical audits if needed)

## Reporting

See root `SECURITY.md`.
