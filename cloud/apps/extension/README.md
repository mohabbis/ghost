# Ghost Workflow Recorder (Chrome extension)

Capture. The user demonstrates a workflow once in their own browser; Ghost
compiles it into typed steps they review and publish.

## Why an extension, and what it deliberately does not do

A Ghost-hosted remote browser is the more elegant answer — recording in the
same environment the worker replays in removes a whole class of production
defect — and it lost on cost and time-to-users. See
`cloud/docs/ARCHITECTURE_DECISIONS.md` §3 for the full comparison and for the
environment gaps this design has to be engineered against.

**It only records.** Execution stays server-side, in Ghost's workers. An
extension that also executed would mean Chrome must stay open and the laptop
awake, and nothing would run overnight — which is most of the value in
back-office work.

## What it captures

Clicks, typing, dropdown selections, form submissions, and navigation
(including SPA `pushState`). For each element it reads the **accessible role
and name** off the live DOM, plus ordered CSS fallbacks.

That is the design's whole point: those are the fields the worker's resolution
chain prefers, so the trace compiles into steps *deterministically* — no model
in the correctness path, and recording works in a deployment with no AI
dependency configured at all.

## What it never captures

- Passwords, one-time codes, card numbers, and anything whose field name,
  label, or `autocomplete` looks like a secret. The value is **not read**; the
  step is marked `sensitive` with a placeholder for the reviewer to fill in.
- Hidden inputs.
- Session cookies.
- Cross-origin iframe contents (`all_frames: false`).
- Coordinates — there is no step field for them, and a coordinate-replayed
  click is exactly what the semantic selector chain exists to avoid.

Redaction happens at capture because that is the only place it can be done
honestly. A trace carrying the value plus a "please ignore this" flag has
already leaked it.

## Install (unpacked)

1. `chrome://extensions` → enable **Developer mode** → **Load unpacked** →
   select this directory.
2. Open the popup → **Connection** → set your Ghost URL and an API token
   created in Ghost under **Settings → Agent credentials** (revocable there at
   any time).
3. **Start recording**, do the task, **Stop and send to Ghost**, then follow the
   review link.

A bearer token rather than the session cookie: the extension is a different
origin, so a `SameSite=Lax` session cookie is not reliably sent on a cross-site
POST, and a token can be revoked from Ghost without touching the browser.

## Trust boundary

Uploads go to `POST /api/agent/recordings` — the *propose* side. The extension
creates a `Recording` whose compiled steps are a proposal. It cannot publish a
workflow, start a run, or approve anything. A human reviews the steps in the
editor and publishes through `POST /api/workflows`, which revalidates them.

## The contract

The trace format is `@ghost/core/recording/trace`; the compiler is
`@ghost/core/recording/compile`. This extension is plain JavaScript and is not
typechecked against them, so `packages/core/src/recording/roundtrip.test.ts`
pins a fixture in exactly the shape `content.js` emits. If that test fails, the
recorder and Ghost have disagreed about the format — fix one of them rather
than relaxing the test.
