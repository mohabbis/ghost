# Claude Desktop ↔ Ghost routine demo (local MCP)

**Audience:** founders / dogfooders showing Ghost after Phase 1 routine MCP (#296)  
**Build:** stock Ghost **v2.0.3** (local stdio). Do **not** claim ChatGPT, marketplace listing, or remote HTTP as ready.  
**Length:** ~8–10 minutes live, or ~3 minutes narrated.

Honest scope (say once):

> Claude Desktop (or Cursor) can **list → preview → request approval**. You approve in Ghost. Only then can the client call **execute** and read a **receipt**. The assistant never bypasses policy, Zones, or the Action Plan runtime.

```text
list_routines → preview_routine → request_routine_approval
       → (you approve in Ghost) → execute_approved_routine → get_run (receipt)
```

---

## Prep

1. Install Ghost v2.0.3; grant Accessibility (and Input Monitoring on macOS) if you will execute a routine.
   Confirm the MCP binary exists at the path Settings copies (`/Applications/Ghost.app/Contents/MacOS/Ghost` on macOS). If that path is missing, point the client `command` at a local `ghost` build that supports `mcp serve` instead.
2. Record and save one short, safe routine (e.g. open Notes / type a fixed line / save). Prefer something reversible and non-financial for the first demo.
3. Open **Settings → Connect an AI assistant (MCP)**:
   - Enable a **pairing code** (recommended).
   - Copy the MCP config snippet into Claude Desktop (`claude_desktop_config.json`) or Cursor (`~/.cursor/mcp.json`).
   - Restart the client so it picks up `ghost mcp serve`.
4. Keep the Ghost window visible — approvals happen **in Ghost**, not in the chat.

Pairing details and config shape: `docs/mcp-integration.md`.

---

## Live walkthrough

### 1. List (names only)

In Claude Desktop / Cursor, ask something like:

> Use Ghost MCP: list my saved routines.

Expect `ghost.list_routines` — **names only**, no events, no typed text.

**Show:** Ghost Settings still paired; client tool result with routine names.

### 2. Preview (redacted steps)

> Preview the routine named "<your routine>". Show the semantic steps and policy outcome. Do not execute.

Expect `ghost.preview_routine` — redacted semantic steps + policy plan. Typed text stays null/redacted.

**Show:** step list in the client; say out loud that secrets never leave Ghost through this tool.

### 3. Request approval (nothing runs yet)

> Request approval to run that exact previewed plan.

Expect `ghost.request_routine_approval` — a pending local approval bound to the **exact plan hash**.

Ghost’s desktop surfaces the pending routine approval (same trust pipeline as Organizer). **Do not** skip the review beat.

### 4. Approve in Ghost

1. Open the pending approval in Ghost.
2. Review steps / risk / policy.
3. Approve locally so Ghost issues a **single-use**, ~5-minute, plan-hash-bound token.

Say: AI proposed; **you** approved; deterministic code will execute only that hash.

### 5. Execute → receipt

In the client:

> Execute the approved routine, then show the run receipt.

Expect:

1. `ghost.execute_approved_routine` — validates the one-shot token + exact hash; runs the Action Plan runtime (`os-control`).
2. `ghost.get_run` — run summary + **execution receipt** when present (verify / recover path).

**Halt fields (MCP):** both execute and `get_run` expose top-level `stopped_early`, `stop_reason`, and a compact `verifications` array (`step_id`, `label`, `expected`, `observed`, `status`) so Claude/Cursor can narrate a verify mismatch without parsing the full receipt. Typed-value payloads in those rows are redacted (same posture as `preview_routine`).

**Show:** sealed receipt in Ghost (and/or the tool payload). Mention undo/recover is still available through Ghost’s normal surfaces — the client does not get a silent delete path.

---

## What not to say

| Avoid | Prefer |
|---|---|
| “ChatGPT can drive Ghost” | Claude Desktop / Cursor over **local stdio** today |
| “Ghost is on the MCP marketplace” | Local `ghost mcp serve` config you paste yourself |
| “The AI runs the routine” | The AI proposes; Ghost executes **after** local approval |
| “Full source-vs-destination reconciliation” | Per-step verification on approved values (roadmap for full recon) |
| Stock HTTP / relay as the demo path | Experimental-only; keep the demo on stdio |

---

## Failure modes worth showing (optional)

- **Pairing on, wrong code** → client cannot initialize tools until the code matches.
- **Execute without approval** → `execute_approved_routine` refuses (no token / hash mismatch).
- **Stale token** → re-preview / re-request; do not reuse an old approval.

---

## Related

- Tool table + pairing: `docs/mcp-integration.md`
- Action Plan shape: `docs/GHOST_2_DEMO.md`
- PH/YC camera script (verify wedge): `docs/launch-demo-script.md`
