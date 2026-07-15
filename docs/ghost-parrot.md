# Ghost Parrot

Ghost Parrot is the suggestion layer for Ghost. It should behave like an assistive preview system, not an always-on observation feature. The product boundary matters because users need to understand when Ghost is observing activity, what signals it uses, and what control they retain.

## Product framing

Ghost Parrot observes only when enabled, looks for repeated local workflow patterns, and produces lightweight recommendation previews. The user stays in control:

- Parrot suggests; it does not act alone.
- Every automation recommendation requires explicit approval before recording, saving, or replaying anything.
- Suggestions should explain why they appeared.
- Suggestions should be dismissible.
- Parrot should be easy to pause, disable, or reset.
- No recommendation should imply that Ghost is reading private content unless the user explicitly enabled that scope.

## Mental model

A good recommendation should feel like:

> You often do this sequence after opening this app. Want Ghost to draft a workflow for review?

Not like:

> I watched everything you did and built an automation. Trust me.

The first is inspectable and permission-bounded. The second implies hidden observation and should not appear in product copy or UI.

## Similarity to assistant previews

Claude, Codex, and other assistants can often produce fitting recommendations because they use conversation context, user preferences, recent actions, and explicit instructions. Ghost Parrot should borrow that pattern locally:

- current app or workflow context
- repeated user-approved actions
- recent dismissed or accepted suggestions
- explicit user preferences
- local workflow history
- safe product boundaries from `docs/core-boundaries.md`

The recommendation engine should prefer transparent signals over mysterious personalization. If Ghost cannot explain why it made a suggestion, it probably should not show it.

## Permission model

Ghost Parrot should have clear modes:

| Mode | Behavior |
|---|---|
| Off | No observation and no suggestions. |
| Manual | Suggestions only after the user records or reviews a workflow. |
| Assistive | Looks for repeated patterns while Ghost is open and proposes draft automations. |
| Focused | Observes only a selected app, window, or workflow session. |

The default should be `Manual` or `Off` until the product earns trust.

## Suggestion lifecycle

1. Observe eligible local signals.
2. Detect a repeated pattern or likely automation opportunity.
3. Generate a short recommendation preview.
4. Show why the suggestion appeared.
5. Ask permission before creating a draft workflow.
6. Let the user accept, edit, dismiss, mute similar suggestions, or disable Parrot.
7. Store only the minimum local feedback needed to improve future suggestions.

## Recommendation object

Recommended shape:

```json
{
  "id": "suggestion_01",
  "title": "Create a workflow for your invoice cleanup sequence?",
  "summary": "You repeated a similar 6-step sequence three times in the last session.",
  "why": [
    "Same app",
    "Similar click and typing sequence",
    "Repeated within 20 minutes"
  ],
  "confidence": 0.78,
  "permission_required": true,
  "actions": ["preview", "draft_workflow", "dismiss", "mute_similar", "turn_off"]
}
```

## Hard rules

- Never auto-run a workflow from a suggestion.
- Never save a generated workflow without user approval.
- Never hide the reason a suggestion appeared.
- Never make disabling Parrot harder than enabling it.
- Never market Parrot as universal computer surveillance.
- Never use cloud sync as a hidden dependency for local suggestions.

## UX copy examples

Good:

- "Ghost noticed a repeated sequence. Preview a draft workflow?"
- "This suggestion is based on three similar actions in the current app."
- "Dismiss suggestions like this."
- "Turn off Ghost Parrot."

Bad:

- "Ghost has learned your behavior."
- "Automatically optimize your entire computer."
- "We detected what you were trying to do."
- "Let Ghost take over."

## Implementation notes

Parrot should sit on top of observer and knowledge modules, not inside the stable recorder/replay core. Keep the stable core constrained and testable. Parrot should remain experimental until it has tests, a permissions screen, transparent explanations, and a reliable off switch.
