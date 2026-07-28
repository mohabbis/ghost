# Ghost as an agent plugin (trust runtime)

**Locked strategy:** Ghost is not another AI agent. It is the **governed
execution runtime that AI agents plug into**.

```text
Agent (propose)  →  Ghost (approve · execute · verify · audit)
```

Claude, Cursor, Codex, ChatGPT, and similar tools may **list workflows, preview
plans, start runs, and observe status**. They **must not approve** sensitive
steps — that stays human-only in the Ghost UI.

Working product name is still **Ghost**; a rename is expected later (generic /
hard to sell). Do not block engineering on naming.

## Why this niche

- Generic agents already exist; competing on “more autonomy” races to the bottom
  on trust.
- Buyers who pay for ops automation need a gate, a verify step, and an audit
  trail — exactly what Ghost’s engine already does.
- Distribution: ride existing agents (MCP / HTTP tools) instead of building a
  standalone chatbot UX first.

## What shipped in this surface

| Piece | Path | Notes |
|---|---|---|
| Tool catalog | `@ghost/core/agent` | Allow-listed tools + explicit forbid list |
| HTTP API | `/api/agent/*` | Session cookie **or** bearer `GHOST_AGENT_API_KEY` |
| MCP stdio | `cloud/apps/mcp` | Thin bridge → HTTP invoke |
| Approval | Ghost web UI only | `POST /api/agent/approvals` → **403** |

### Agent tools (allow list)

- `list_workflows` — org workflow metadata
- `get_workflow` — latest version steps (preview)
- `start_run` — enqueue run (sensitive steps still gate)
- `get_run` — status / steps / approvals
- `list_pending_approvals` — read-only; tell the human to open `/runs/[id]`

### Forbidden

Anything that approves or rejects (`approve_run`, `reject_run`, …). Invoke and
MCP return an error; there is no approve tool in the catalog.

## Local setup (MCP + API key)

1. Run Ghost Cloud (`pnpm dev` in `cloud/`).
2. Sign in once, create a demo workflow if needed.
3. Copy your **org id** and **user id** (from the DB, or Settings once exposed):

```bash
# example — adjust to your docker/psql
docker compose exec postgres psql -U ghost -d ghost \
  -c 'select m."orgId", m."userId", u.email from "Membership" m join "User" u on u.id = m."userId";'
```

4. Add to `cloud/.env`:

```bash
GHOST_AGENT_API_KEY="$(openssl rand -hex 32)"
GHOST_AGENT_ORG_ID="<org cuid>"
GHOST_AGENT_USER_ID="<user cuid>"
```

Restart web so it picks up the env. Turbo `globalEnv` includes these keys.

5. Point Cursor (or another MCP client) at the bridge:

```json
{
  "mcpServers": {
    "ghost": {
      "command": "pnpm",
      "args": ["--filter", "@ghost/mcp", "exec", "tsx", "src/index.ts"],
      "cwd": "/absolute/path/to/repo/cloud",
      "env": {
        "GHOST_API_URL": "http://localhost:3000",
        "GHOST_AGENT_API_KEY": "<same key as web .env>"
      }
    }
  }
}
```

6. Ask the agent: “List Ghost workflows and start the demo run.” When it hits
   `AWAITING_APPROVAL`, approve in the browser — not in the agent chat.

### curl smoke

```bash
export KEY=… BASE=http://localhost:3000
curl -s -H "Authorization: Bearer $KEY" "$BASE/api/agent" | jq .
curl -s -H "Authorization: Bearer $KEY" -H 'content-type: application/json' \
  -d '{"name":"list_workflows","arguments":{}}' "$BASE/api/agent/invoke" | jq .
curl -s -X POST -H "Authorization: Bearer $KEY" -H 'content-type: application/json' \
  -d '{"name":"approve_run","arguments":{}}' "$BASE/api/agent/invoke"
# → 403
```

## Auth model (honest / early)

| Path | When |
|---|---|
| Session cookie | Same browser session as the dashboard |
| Bearer `GHOST_AGENT_API_KEY` | Local MCP; identity = `GHOST_AGENT_ORG_ID` + `GHOST_AGENT_USER_ID` |

Per-org rotatable API keys in Postgres are **follow-up**. Until then, treat the
env key as a local/dev secret — not multi-tenant SaaS credentials.

## Non-goals for this surface

- Letting an agent approve its own gated steps
- Shipping ChatGPT remote MCP / OAuth connector theater before the local loop works
- Replacing the web approval UI
- Broad connector APIs (still after recording)

## Related

- Trust principles: `docs/trust-pipeline.md`, `AGENTS.md`
- Engine: `cloud/docs/PHASE_1_PLAN.md`, `cloud/docs/CURSOR_HANDOFF.md`
- Legacy desktop MCP (historical): `docs/legacy/mcp-integration.md`
