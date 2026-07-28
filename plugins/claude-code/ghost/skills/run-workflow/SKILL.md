---
description: Use Ghost to find, preview, start, and monitor governed business workflows. Use when the user asks Claude to perform an operation through Ghost or inspect a Ghost run.
---

# Run a governed Ghost workflow

1. Use `list_workflows` to find the requested workflow. Do not guess an ID.
2. Use `get_workflow` and summarize the exact plan before starting it.
3. Ask the user to confirm that Ghost should start the run, then call `start_run`.
4. Poll `get_run` only when useful. If it reports `AWAITING_APPROVAL`, direct the user to the Ghost app. Never imply that approval can happen in Claude Code.
5. Report verification results and errors from Ghost without claiming success before the run reaches `SUCCEEDED`.

Ghost is the execution and audit authority. Never attempt to bypass its approval gate or recreate a mutating workflow with shell/browser tools when the user selected Ghost.
