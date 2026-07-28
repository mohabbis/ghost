# Trust pipeline

Every meaningful Ghost Cloud operation:

```text
Intent → Plan → Policy check → User approval → Execution → Audit log → Undo/Recover
```

In the cloud MVP this maps to:

| Stage | Where |
|---|---|
| Intent | User triggers a run / later: a recording |
| Plan | Immutable `WorkflowVersion` steps |
| Policy | `classifyStep` in `@ghost/core` (deterministic) |
| Approval | `Approval` row + UI; run `AWAITING_APPROVAL` |
| Execution | Worker Playwright / later connectors |
| Verify | Per-step assertions + screenshots |
| Audit | Hash-chained `AuditEvent` per org |
| Recover | Cancel mid-run; later: undo for reversible connectors |

## Rules

1. AI never decides whether a step is sensitive.
2. Approval is a one-way gate the engine cannot skip.
3. A run is not “done” because a click happened — verification must pass.
4. Secrets never land in logs, screenshots, or model prompts.
5. Every mutating API route / job is org-scoped via auth.

Desktop Organizer had the same *principles* with different mechanics; see
`legacy/` only when maintaining that code.
