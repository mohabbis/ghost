# macOS Automation Architecture

Canonical direction for Ghost’s macOS operating layer. This is an implementation
plan for the Routines / Action Plan data plane — not a rewrite of the trust core.

Related:

- responsibility boundary today: [`native-macos-preview.md`](native-macos-preview.md)
- current click resolution chain: [`target-resolution.md`](target-resolution.md)
- Action Plan runtime: [`GHOST_2_DEMO.md`](GHOST_2_DEMO.md)
- product contract: [`AGENTS.md`](../AGENTS.md)

## Product constraint

Ghost is a local-first automation product with a trust pipeline:

```text
Intent -> Plan -> Policy check -> User approval -> Execution -> Audit log -> Undo path
```

macOS APIs are the **execution data plane**. They must not become a second policy
engine, a silent observer, or a path around approval. AI may propose steps;
deterministic Ghost code resolves, checks policy, executes only approved plans,
and records evidence.

## Three capability layers

Ghost needs three distinct macOS capability layers. Prefer them in this order.

### 1. Semantic UI automation (Accessibility)

Use `AXUIElement` first. It exposes application UI hierarchies, attributes,
positions, supported actions, and state changes. Prefer actions such as
`AXPress` or setting an accessibility value rather than clicking coordinates.

Existing surface: `native/macos/GhostAXHelper.swift` and the Rust fallback in
`src-tauri/src/platform/macos.rs`.

### 2. Input observation and replay (Core Graphics events)

Core Graphics events can represent and post keyboard and mouse activity. This is
appropriate for:

- recording;
- fallback replay when AX cannot invoke the control;
- drag operations;
- shortcuts;
- controls that have no usable accessibility action.

Core Graphics remains the right layer for event taps, event creation/posting,
cursor coordinates, and display geometry. It is **not** Ghost’s primary modern
screen-capture framework.

### 3. Visual fallback (ScreenCaptureKit + Vision)

Use **ScreenCaptureKit** for modern window/display capture. It supports choosing
specific displays, applications, and windows and delivers frames with associated
metadata. Use **Vision** for local OCR and text bounding boxes.

Capture is user-scoped and feature-gated. No ambient observation. Screenshot
fragments for template matching remain opt-in (see
[`target-resolution.md`](target-resolution.md)).

Occasional still-image utilities may still use Core Graphics helpers; new
window/display capture work should target ScreenCaptureKit.

## Resolution order

Every UI step resolves in this order:

```text
AX element lookup
    ↓ failure or insufficient AX quality
Vision / OCR or visual matching
    ↓ failure
Coordinate replay
    ↓
Verified postcondition
```

Humans traditionally start with coordinate clicks because they enjoy debugging
software that breaks whenever someone moves a window eight pixels. Ghost does
not.

Today’s Tauri replay chain (`docs/target-resolution.md`) already prefers
descriptor match → window-relative → spiral → template → coordinates. The native
macOS subsystem should converge on the same semantics, with AX actions preferred
over synthetic clicks whenever the tree supports them.

## Accessibility quality, not framework absolutes

Do **not** treat framework labels as capability facts.

Incorrect absolute:

> Electron and Flutter apps do not expose structured accessibility trees.

Correct rule:

> Many Electron applications expose usable accessibility trees, depending on how
> the app was built and whether controls preserve semantic roles. Flutter’s macOS
> accessibility support also varies by implementation. Ghost inspects the AX tree
> first, scores its quality, and only then switches to vision.

Use visual recognition when the target application exposes an **incomplete,
unstable, or insufficient** accessibility hierarchy — not merely because the
process happens to be Electron or Flutter.

AX quality signals (illustrative, not exhaustive):

- presence of role / title / identifier;
- stable identifier across runs;
- actionable attributes (`AXPress`, settable `AXValue`);
- hierarchy depth and sibling ambiguity;
- match uniqueness for the declared locator.

## What to study (execution semantics, not product architecture)

Appium and WebDriverAgent are weak architectural references for a general macOS
automation operating layer. WebDriverAgent is primarily associated with
Apple-platform application testing (especially iOS); it is not Ghost’s reference
architecture.

Better systems to study for **execution semantics**:

- AppleScript and System Events;
- macOS UI testing through XCTest;
- Playwright’s locator and retry model;
- Microsoft UI Automation patterns;
- Sikuli-style visual matching;
- accessibility inspectors and AX clients.

Study how they locate, wait, retry, and verify — not how they package a test
product.

## Swift / Rust ownership split

Swift does **not** replace Rust.

```text
SwiftUI / AppKit macOS shell
        │
        ├── permission onboarding
        ├── Accessibility bridge
        ├── ScreenCaptureKit
        ├── Vision OCR
        ├── window and application state
        └── native notifications
        │
        ▼
Narrow Swift ↔ Rust bridge
        │
        ▼
Rust trusted execution core
        ├── plans
        ├── policy evaluation
        ├── approval state
        ├── workflow representation
        ├── retries and timeouts
        ├── audit logging
        ├── undo journals
        ├── storage
        └── MCP
```

| Layer | Owns | Must not own |
|---|---|---|
| Swift / AppKit | native macOS integration, AX/SCK/Vision adapters, permission UX, window/app state | policy decisions, plan approval authority, trusted mutations, audit construction |
| Bridge | versioned commands/events, process lifecycle | presentation state or product rules |
| Rust core | plans, policy, approval, retries/timeouts, execution orchestration, audit, undo, storage, MCP | SwiftUI rendering, raw ScreenCaptureKit session management |

Rewriting the trusted engine in Swift would mostly produce a different collection
of bugs wearing an Apple-approved sweater. Keep the split above.

Grounding today: [`apps/macos/`](../apps/macos/),
[`docs/native-macos-preview.md`](native-macos-preview.md),
[`native/macos/GhostAXHelper.swift`](../native/macos/GhostAXHelper.swift).

## Semantic step model

Every automation step should be represented semantically (shared shape across
Swift capture and Rust execution — exact Rust types may evolve):

```text
AutomationStep {
    target_app,
    target_window,
    locator,
    action,
    preconditions,
    timeout,
    retry_policy,
    postconditions,
    fallback_strategy,
    risk_level
}
```

### Locator strategies

```text
enum Locator {
    Accessibility {
        role: Option<String>,
        title: Option<String>,
        identifier: Option<String>,
        value: Option<String>,
        ancestor_path: Vec<AXConstraint>,
    },
    Text {
        content: String,
        region: Option<Rect>,
    },
    ImageTemplate {
        template_id: String,
        confidence: f32,
    },
    Coordinates {
        point: NormalizedPoint,
    },
}
```

### What to persist at capture time

Ghost must not save only raw mouse coordinates. Persist enough to relocate after
windows move or layouts change:

- application bundle identifier;
- process identifier at execution time (ephemeral; do not treat as durable identity);
- window title and bounds;
- accessibility role;
- label or title;
- identifier when available;
- surrounding hierarchy;
- normalized coordinates;
- screenshot fragment only when explicitly permitted;
- expected result after the action.

`ElementInfo` already carries part of this (`docs/target-resolution.md`). Native
capture should extend toward the full set above without requiring Screen
Recording for the AX-first path.

## Reliability requirements

A resilient step should behave like this:

1. Confirm the target application is running.
2. Activate or focus the intended window.
3. Wait for declared preconditions.
4. Resolve the target through AX.
5. Fall back to OCR or visual matching when allowed.
6. Verify the target remains valid.
7. Run the policy check.
8. Request approval when the action mutates state.
9. Execute the action.
10. Verify the expected postcondition.
11. Retry only safe and idempotent operations.
12. Record the result, evidence, and undo information.

### Retries are not universal

Retrying “open menu” is reasonable. Retrying “submit payment,” “send message,”
or “delete file” can duplicate irreversible actions.

Policy must distinguish at least:

| Class | Retry default | Examples |
|---|---|---|
| Read-only | Allowed | inspect, OCR, list matches |
| Reversible mutation | Cautious / bounded | move file with undo, focus window |
| Externally consequential | Deny automatic retry | send message, submit form, upload |
| Destructive / irreversible | Never auto-retry | delete, overwrite, payment submit |

Map these onto Ghost risk classes (`safe-read`, `sensitive-read`,
`local-mutate`, `external-mutate`, `os-control`, `experimental`) and refuse to
market UI postconditions as proof of business effect (see ADR-0007).

## Permission architecture

Implement a dedicated **permission coordinator** rather than scattering checks
across features.

```text
enum GhostPermission {
    accessibility
    screenRecording
    inputMonitoring
    notifications
    automation
}
```

For each permission, track:

- current status;
- why Ghost needs it;
- which feature depends on it;
- whether the app must restart;
- deep link or instructions for System Settings;
- last successful validation;
- degraded behavior when denied.

### Partial operation is required

“Grant everything before continuing” is lazy permission design. The product must
work partially without every permission:

| Denied | Degraded but available |
|---|---|
| Screen Recording | AX-only automation and inspection |
| Accessibility | Inspection/visual analysis where otherwise permitted; no AX actions |
| Input Monitoring | Recording disabled; approved replay may still work where permitted |

Organizer (folder Zones → plan → approve → execute → undo) must remain usable
without Input Monitoring or Screen Recording.

## Build order

Build the macOS automation subsystem in this order. Do not skip ahead to MCP or
visual matching before AX targeting and permissions are solid.

1. Native Swift package for Accessibility inspection and actions
2. Reliable application and window targeting
3. Permission coordinator and onboarding UI
4. Semantic locator format shared with Rust
5. ScreenCaptureKit still-frame and stream capture ✅ (still + bounded stream sample;
   not ambient continuous observation)
6. Vision OCR fallback
7. Verified execution with preconditions and postconditions
8. Visual template matching only where OCR and AX fail
9. Evidence capture, audit records, and undo integration
10. MCP exposure of already-trusted plans (pairing + approval queue; no bypass)

## Honest status (do not overclaim)

Already present:

- AX helper ops (`resolve_target`, `list_matches`, `activate_element`, `set_value`, …)
  with exact bundle-id / app-name targeting, optional `window_title` search root,
  `identifier` matching, AXPress-first activate, and per-match `ax_quality`;
- ScreenCaptureKit **still-frame** ops on the same helper (`capture_permission_status`,
  `capture_still`) — Screen Recording only; no Accessibility required;
- ScreenCaptureKit **bounded stream** sample (`capture_stream_latest`): short-lived
  SCStream (default ≤400 ms / ≤3 frames, hard-capped at 2 s / 8 frames), writes the
  latest complete frame, then stops — request-scoped only, **not** ambient observation;
- Rust `runtime/capture.rs`: `capture_still` / `capture_stream_latest` /
  `capture_latest_frame_bytes` (stream → still → legacy) for OCR fallback;
- Rust `runtime/vision_fallback.rs`: when AX resolve fails or quality is insufficient
  **and** `UiTarget.title` is set, latest-frame capture → OCR → unique text match →
  coordinate click for `focus_target`;
- Rust shared locator + AX quality types (`runtime/locator.rs`) and extended
  `UiTarget` (`identifier`, `bundle_id`, `window_title`);
- native `PermissionService` coordinator with per-permission why/degraded-mode
  metadata (Accessibility + Screen Recording probed; Input Monitoring /
  Notifications / Automation still honest `unknown` until probes land);
- Rust macOS platform backend (CGEventTap + AX inspection);
- descriptor-first resolution chain with template and coordinate fallbacks;
- on-device OCR for **user-supplied** images (`run_ocr_on_image`) and for
  vision-fallback frames;
- Action Plan runtime with verify/receipt/undo;
- native SwiftUI Organizer scaffold over the Rust bridge.

Also present (partial item 7):

- `app_running` precondition before semantic focus/set_value (skipped when helper missing);
- `set_value` vision fallback (OCR-click label → type) when AX is insufficient and `title` is set;
- semantic postconditions via `verify_postcondition` (AX verify, else OCR text presence);
  UI failures are recorded honestly but do not hard-stop the plan (ADR-0007).

Not yet the architecture above:

- long-lived / continuous stream sessions (product observation) — only bounded samples exist;
- full `AutomationStep` runtime fields (timeouts / retry policy wired end to end);
- Input Monitoring / Notifications probe implementations;
- template matching only after OCR+AX fail as a dedicated Routines strategy;
- evidence / audit fields for which capture strategy (still vs stream) served a UI step.

Until those land, marketing and README copy must stay within what the repo
supports (`AGENTS.md` rule 10). Do not market bounded stream sampling as always-on
screen observation or as proof of business effect.
