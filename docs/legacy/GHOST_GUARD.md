# Ghost Guard Safety Layer

Ghost Guard is the local audit and safety layer for Ghost workflows. It is intentionally deterministic first: the app must protect user data even when no external AI provider is configured.

## Implemented now

- Suppresses keyboard capture after a click into fields that look like passwords, passcodes, OTPs, API keys, payment fields, or other sensitive inputs.
- Drops secret-like keystrokes before they reach the workflow buffer or disk.
- Runs a local workflow audit before replay and save.
- Blocks replay/save when stored credential-like input is detected.
- Requires extra confirmation for sensitive apps, destructive actions, and other high-risk steps.
- Keeps prototype tools behind an Experimental section so the core flow stays focused on record, audit, replay, and save.

## Next features to add

1. **Sensitive app blocklist/allowlist UI** — let users explicitly block password managers, banking apps, private messaging, healthcare portals, and system settings.
2. **Manual checkpoint steps** — replace secrets with “pause and let me type this manually” checkpoints.
3. **Dry-run replay** — highlight targets and show intended keystrokes without executing them.
4. **Per-step debugger** — show pending/running/succeeded/failed/blocked status for each replay step.
5. **Execution trace log** — persist local replay results with failure causes and no sensitive values.
6. **Guard policy settings** — choose strict, balanced, or permissive replay policies.
7. **Local redaction review** — show exactly what was suppressed after recording without exposing the secret.
8. **Real AI audit provider** — once configured, use an LLM only to summarize risks and suggest safer rewrites; never send raw secrets or screenshots by default.
