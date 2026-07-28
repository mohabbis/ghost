# Core boundaries (cloud)

## Stable (defines product behavior)

- Workflow versions + typed steps (`@ghost/core` Zod schema)
- Deterministic sensitive-action classifier
- Run state machine (gate / resume / cancel)
- Playwright execution of browser steps
- Per-step screenshots + verification
- Hash-chained audit events
- Org-scoped Auth.js sessions

## Not yet / keep gated

- Browser recording → editable steps (Phase 2)
- Real connector execution (`apiCall` / `sendEmail` are reserved no-ops)
- Persistent per-org browser profiles / stored cookies
- SSE/WebSocket live updates (polling is fine)
- Self-serve billing

## Legacy desktop

Tauri command modules, Organizer, MCP pairing, experimental desktop AI — **out of
commercial scope**. If you touch that code, read `docs/legacy/` and do not expand
its product surface without an explicit decision.
