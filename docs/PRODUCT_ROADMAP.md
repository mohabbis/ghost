# Ghost Product Roadmap

Ghost should become a practical desktop automation assistant, not a vague AI mascot. The strongest version of the product is simple: watch repetitive desktop work, turn it into safe reusable workflows, and help users run those workflows with confidence.

This roadmap keeps the ambition high while avoiding fake magic. The app should feel intelligent because it is reliable, transparent, and useful, not because the copy says “AI” twelve times and hopes nobody checks.

## Product thesis

Ghost is a cross-platform desktop automation layer for macOS and Windows.

It observes user-approved desktop actions, records repeatable workflows, enriches them with UI element metadata, and replays them with guardrails. Over time, Ghost should detect repeated patterns and suggest automations before the user manually records them.

The revolutionary part is not “AI clicks buttons.” The revolutionary part is giving normal users a safe, local-first way to automate messy work across apps that do not expose clean APIs.

## Current honest positioning

Ghost is early-stage. It has a working foundation for native input recording, workflow replay, workflow storage, and platform-specific backends. Several advanced features exist in code or interface form, but they should be treated as prototype-grade until they are tested across real apps and real users.

Recommended public wording:

> Ghost is an early-stage desktop automation app for macOS and Windows. It can record and replay workflows, inspect UI elements, and experiment with smarter automation suggestions. The foundation works, but reliability and cross-app robustness are still being actively improved.

Avoid claiming that AI automation, cloud sync, enterprise audit logging, or proactive observation are production-ready until they have tests, stable UI flows, and documented limits.

## Near-term priorities

### 1. Make recording and replay boringly reliable

This matters more than every “AI” feature.

Must-haves:

- Record left click, right click, keyboard input, scroll, and timing consistently.
- Preserve both coordinates and semantic metadata when available.
- Replay with pause, cancel, speed control, and clear failure states.
- Show exactly what Ghost captured before letting the user run it again.
- Add a dry-run mode that highlights intended targets before executing.

Success metric:

- A user can record a 10-step workflow in Finder, Chrome, Excel, or a browser form and replay it successfully after minor window movement.

### 2. Build a workflow debugger

Automation fails. The difference between a toy and a serious product is whether the user can understand why.

Needed UI:

- Timeline of recorded events.
- Per-step target preview.
- Coordinates, app name, role, label, and fallback strategy.
- Step-by-step replay mode.
- “Retry from failed step.”
- Failure messages that explain whether the issue was permissions, missing window, moved element, timing, or unknown target.

This is the feature that makes Ghost feel trustworthy instead of haunted in the boring software sense.

### 3. Add templates for real workflows

Do not start with generic automation. Start with repeatable, obvious jobs.

Good first templates:

- Copy data from one app into a web form.
- Open the same set of apps or tabs for a daily routine.
- Rename or organize files using a predictable pattern.
- Fill repeated CRM/admin fields.
- Download, move, and rename reports.
- Perform QA checks across a repeated web flow.

Each template should include:

- What it automates.
- What apps it works best with.
- What can break it.
- Whether it uses keyboard, mouse, semantic UI lookup, or both.

### 4. Keep AI constrained and useful

AI should not directly control the computer blindly. That is how you get a very confident robot deleting the wrong folder, which is apparently the future people keep asking for.

Good AI uses:

- Name a recorded workflow.
- Summarize what a workflow does.
- Detect repeated patterns from local history.
- Suggest where a workflow can be shortened.
- Explain why a replay failed.
- Convert a user prompt into a draft workflow that requires review before execution.

Bad AI uses for now:

- Fully autonomous background control.
- Running across sensitive apps without explicit confirmation.
- Silent observation by default.
- Cloud-dependent automation for local desktop actions.

Rule:

> AI may suggest and draft. The user approves before execution.

### 5. Make privacy a core feature

Ghost watches desktop activity. That is powerful and creepy unless handled carefully.

Required principles:

- Local-first by default.
- Clear recording indicator.
- No silent background capture unless the user explicitly enables observer mode.
- App allowlist and blocklist.
- Sensitive app warnings for password managers, banking, healthcare, private messaging, and system settings.
- Plain-language data controls.
- Export and delete all local data.

Privacy should be part of the brand, not buried below a footer link nobody reads because apparently that is how software earns trust now.

## Product modes

### Record Mode

The user manually starts recording, performs a task, stops recording, reviews the captured workflow, names it, and saves it.

This should be the default mode until Ghost is trustworthy.

### Replay Mode

The user selects a saved workflow and runs it. Ghost shows visible progress and allows cancel/pause immediately.

### Debug Mode

The user runs a workflow one step at a time, sees each target, and can edit or remove failed steps.

### Observer Mode

Ghost looks for repeated activity patterns, but only after explicit opt-in. It should produce suggestions, not run actions.

### Prompt Mode

The user describes a task. Ghost drafts a workflow using the existing event schema, then asks the user to review and test it.

## Technical roadmap

### Phase 1: Stabilize the engine

- Normalize event schemas across macOS and Windows.
- Split mouse down/up from logical click events clearly.
- Add timestamps at native capture time, not only bridge arrival time.
- Add integration tests for serialization/deserialization of workflows.
- Add replay cancellation tests.
- Add permission-state tests where possible.
- Keep CI compile-only packaging separate from release packaging.

### Phase 2: Improve target resolution

- Store multiple locator strategies per event:
  - semantic UI role/name/app,
  - window title,
  - relative coordinates within window,
  - absolute fallback coordinates,
  - optional screenshot/visual checkpoint.
- Re-resolve UI elements before clicking.
- Fall back gracefully when semantic lookup fails.
- Explain fallback choices in the UI.

### Phase 3: Workflow editor

- Editable event list.
- Delete/reorder steps.
- Insert wait steps.
- Add target confirmation steps.
- Add variable placeholders.
- Add per-step retry/backoff.
- Add test replay for a single step.

### Phase 4: Practical intelligence

- Pattern detection from repeated saved workflows and opt-in observer sessions.
- Workflow naming and summaries.
- Failure explanations.
- Suggested simplifications.
- Natural-language-to-draft workflow generation with mandatory review.

### Phase 5: Distribution quality

- Stable signed macOS builds.
- Windows code signing.
- Clear release notes.
- Download checksums.
- Installer smoke tests.
- Update channel strategy.
- Separate public marketing claims from experimental internal features.

## Safety and trust guardrails

Ghost should never execute dangerous actions invisibly.

Minimum guardrails:

- Always-visible recording state.
- Always-visible replay state.
- Emergency stop shortcut.
- Confirmation before running workflows that touch sensitive apps.
- Confirmation before destructive actions, including delete, send, purchase, submit, transfer, or install.
- User-editable blocklist.
- Local audit log of workflow executions.

## What not to build yet

Avoid these until the core replay loop is reliable:

- Team workspaces.
- Enterprise cloud sync.
- Marketplace.
- Fully autonomous agent mode.
- Browser extension.
- Mobile app.
- Overbuilt dashboards.

These may be useful later, but right now they distract from the thing that matters: can Ghost watch a task, understand enough of it, and run it back without embarrassing itself?

## Suggested public feature hierarchy

Use this order in the README and website:

1. Record workflows.
2. Replay safely.
3. Understand what was clicked.
4. Edit and debug workflows.
5. Get smart suggestions.
6. Generate draft automations from prompts.

This is more believable than leading with “AI parrot” and then asking users to clear macOS quarantine manually.

## Demo milestones

### Demo 1: Daily startup routine

Record opening specific apps or tabs, then replay it. Low risk, easy to understand.

### Demo 2: File organization

Record moving/renaming files in Finder or Explorer. Shows real desktop automation.

### Demo 3: Browser admin workflow

Record filling a repeated form. Use dry-run and step confirmation to show safety.

### Demo 4: Replay recovery

Intentionally move the window, then show Ghost resolving the target semantically or explaining why it fell back.

## Definition of a real beta

Ghost is beta-ready when:

- macOS and Windows installers build consistently.
- The app can record and replay common workflows in at least three real apps on each OS.
- Users can inspect and edit steps.
- Permission flows are clear.
- Privacy controls are visible.
- Failed workflows explain what went wrong.
- README and website clearly label experimental features.

Until then, call it an alpha or technical preview. That is not weakness. That is how adults avoid lying to users with gradients.
