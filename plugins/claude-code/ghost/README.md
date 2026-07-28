# Ghost for Claude Code

This plugin connects Claude Code to Ghost's governed execution runtime. Claude
can inspect workflows, start runs, and report status. It cannot approve gated
steps; approval stays in the authenticated Ghost app.

## Setup

1. Sign in to Ghost and open **Settings → Claude Code and agent access**.
2. Create a credential and copy it when shown.
3. Export the credential before starting Claude Code:

   ```bash
   export GHOST_API_URL="https://your-ghost.example.com"
   export GHOST_ACCESS_TOKEN="ghost_agent_…"
   claude --plugin-dir ./plugins/claude-code/ghost
   ```

4. Ask Claude to list or run a Ghost workflow, or invoke
   `/ghost:run-workflow` explicitly.

Treat the credential like a password. Revoke it from Ghost Settings when a
device no longer needs access.
