# Ghost Cloud — architecture decisions

Four decisions that were open, and the audit that motivated closing them. This
is a decision record, not a roadmap: each entry states what was chosen, what was
rejected, and what it costs to defer. Where something is still open, it says so
rather than implying a plan exists.

Written against `cloud/` at the merge of PR #403. Companion docs:
[`CURSOR_HANDOFF.md`](CURSOR_HANDOFF.md) (where we are),
[`DEPLOY.md`](DEPLOY.md) (how it runs), [`PRIOR_ART.md`](PRIOR_ART.md) (what the
engine borrows).

---

## 1. HarnessRouter is development tooling, not runtime — **decided**

### The decision

HarnessRouter is used to *build* Ghost. It is not part of what Ghost *runs*.

```text
Developer → HarnessRouter → builds and maintains Ghost
                                     │
                                     ▼
                        Ghost production platform
                                     │
                                     ▼
                  Ghost workers execute customer workflows
```

Customers never receive HarnessRouter credentials, never see a HarnessRouter
session, and never depend on HarnessRouter availability. Production must boot
and run with no `HR_API_KEY` present.

### What prompted it

The recording compiler (`apps/web/src/lib/recording-compiler.ts`) sent an
uploaded trace to `api.harnessrouter.ai` and read back proposed steps. Two
consequences, both now moot but worth recording because they are the reason the
boundary exists:

**Shared tenancy.** `HR_API_KEY` is process-wide, so every organization's traces
and compile sessions shared one HarnessRouter Workspace. Ghost's own routes are
org-scoped and refuse cross-tenant access — verified across detail, compile,
continue, cancel and stream — but that is Ghost enforcing isolation on its own
surface, not isolation inside the vendor. Anyone holding the key could enumerate
every tenant's sessions there.

**Data egress.** A trace is a recording of someone doing real work in real
systems. HAR files and Playwright traces carry request and response bodies,
`Cookie` and `Authorization` headers, and typed input. The compiler prompt
instructs the model not to copy secrets into its *output*; that protects the
artifact, not the transfer. The trace was uploaded whole, with no redaction, to
a vendor the customer never contracted with.

A per-organization-workspace design would have addressed the first and not the
second, at the cost of provisioning logic, per-customer credential management,
and a real increase in onboarding complexity. Removing the dependency from the
runtime addresses both and removes the provisioning work entirely. That is why
the comparison table this document was originally going to contain is not here:
the question stopped being live.

### What it means concretely — **implemented**

- Recording compile sits behind `WorkflowCompiler`
  (`apps/web/src/lib/compiler/types.ts`): `start` / `resume` / `poll` /
  `cancel`, with an opaque `CompileJob` handle and a `CompileProgress` result
  carrying steps already validated against `@ghost/core/schema/step`.
- `harness-router-compiler.ts` is the only implementation. It and
  `lib/harness-router.ts` are the only files that know a harness or a session
  exists. Deleting them and one line of `lib/compiler/index.ts` removes the
  dependency, and only compilation can break.
- `workflowCompiler()` returns `null` when `HR_API_KEY` is unset. Routes render
  that as **503 "not enabled"** through one shared helper — an expected
  production state, not a server error.
- `Recording.harnessResponseId` / `harnessSessionId` are renamed to
  `compileJobId` / `compileSessionId`, so no vendor vocabulary remains in the
  data model.
- `compiler-optional.test.ts` asserts the app resolves no compiler, throws a
  typed catchable error, and renders 503 — plus a structural check that
  nothing in `apps/worker` or `packages/core` imports the compiler at all, so
  an unset key can never become a boot failure for the execution path.
- Verified: `pnpm build` succeeds with `HR_API_KEY` unset; 365 tests pass.

Production runs with the feature off, which costs nothing today: there is no
capture client producing traces yet (§3), so the compile path has no production
input to serve.

### What is still open

If and when compile returns to production, it needs an execution home Ghost
controls, with redaction applied *before* egress rather than requested of a
model, and a per-organization opt-in. That is a product decision with a vendor
attached; it should not be made as a side effect of restoring a feature.

---

## 2. Enterprise authentication — **designed, deliberately deferred**

> **Status: do not build this yet.** It is written down so that the schema
> decisions made before it arrives do not foreclose it — three of them below
> are cheap now and expensive later. Nothing here should be implemented until
> the record → edit → publish → run → approve → verify loop works end to end
> for one real workflow (§5). SSO is what a buyer asks for *after* they want
> the product; it has never been what made them want it.


Nothing exists today. Sign-in is GitHub OAuth plus a dev-only credentials
provider (`apps/web/src/auth.ts`). Sessions are 12-hour JWTs; the `Session` and
`Account` tables exist but no adapter writes them. TOTP two-factor is real and
enforced in middleware. Roles are a two-tier boolean (`isOrgAdmin`) that gates
member management and nothing else.

### The shape, when it is time

**Support enterprise IdPs over generic OIDC, per organization. Defer SAML until
a paying customer requires it. Defer SCIM until seat counts justify it.**

Okta, Microsoft Entra ID, and Google Workspace all speak OIDC natively. An
OIDC-only implementation covers the three IdPs named as targets with one code
path, no bridge service, and no new subprocessor — which is the same principle
as §1. SAML matters for older or unusually configured deployments; it is a real
requirement eventually, and a speculative one now.

Rejected: adopting a hosted identity vendor (WorkOS and similar) to get SAML,
OIDC and SCIM at once. It is genuinely faster, and it puts a third party in the
authentication path for every tenant, priced per connection. Having just removed
one vendor from the runtime, adding one to the login path needs a stronger
reason than convenience.

When SAML does arrive, the shape that preserves this decision is a self-hosted
SAML-to-OIDC bridge (BoxyHQ SAML Jackson, Apache-2.0) presented to Auth.js as
one more OIDC connection — not a second parallel auth stack.

### Schema changes — three of these are worth making early

Additive, in the order they are needed. `User.externalId`, `User.sessionEpoch`
and the `Membership.role` default are the ones that get expensive to retrofit
once there is production data; the rest can wait for the feature.

```text
Organization
  domains            String[]     verified email domains, for home-realm discovery
  requireSso         Boolean      when true, password/OAuth fallback is refused
IdentityProvider     new model
  orgId, protocol (OIDC|SAML), issuer, clientId,
  clientSecretRef (envelope-encrypted, never a raw value),
  enabled, createdAt
User
  externalId         String?      stable IdP subject; email is not a durable key
  sessionEpoch       Int          bumped to invalidate outstanding JWTs
Membership
  provisionedBy      enum(MANUAL|SSO|SCIM)
Role
  add VIEWER and APPROVER         (see audit P1-5)
```

Three of these are load-bearing in ways that are easy to miss:

- **`User.externalId`.** Today `ensureUserOrg` upserts on email. An IdP that
  changes a user's address, or a deprovision-then-rehire, breaks that identity.
  SCIM later has nothing stable to address.
- **`User.sessionEpoch`.** A 12-hour JWT cannot be revoked. SSO deprovisioning
  and SAML single-logout both need to invalidate a live session; the middleware
  must compare an epoch claim against the database. Without it, "we removed
  their access" is true for up to twelve hours.
- **`Membership.role` currently defaults to `OWNER`.** Any JIT- or
  SCIM-provisioned user created without an explicit role becomes an org owner.
  This must become an explicit argument before any automatic provisioning path
  exists.

### Rollout

1. **Per-org OIDC connections + enforced SSO + session epoch.** Admin configures
   issuer and client credentials; users on a verified domain are routed to their
   IdP. JIT-provision membership on first login with an explicit non-owner role.
2. **Group-to-role mapping**, once roles are more than two tiers.
3. **SCIM 2.0** (`/Users`, `/Groups`) — driven by a customer with enough seats
   that manual deprovisioning is a real risk.
4. **SAML** via a self-hosted bridge, driven by a customer whose IdP requires it.

Steps 1 and 2 are worth building on a credible pipeline. Steps 3 and 4 are worth
building against a signed contract, not a hypothesis.

---

## 3. Capture architecture — **decided: Chrome extension for v1**

The largest open product decision, now closed. Today there is no capture at
all: users upload a trace file produced by some external means, and an LLM
infers steps from it. `RecordingStatus.ACTIVE` is never set because nothing
records. The manual upload form is scaffolding, not a product.

### The decision

**Ship a Chrome extension.** A Ghost-hosted remote browser is the more elegant
architecture and it lost on cost and time-to-users: running Chrome at scale is
infrastructure engineering, and the extension puts a real capture path in front
of people far sooner.

The rest of this section records why the remote browser was the technically
stronger answer, because those are the specific defects the extension now has
to be engineered against — not an argument to revisit the decision.

### What the extension gives up: record where you replay

Ghost replays in headless Chromium on a worker, with a sealed session blob and
no user profile. An extension records in the operator's real Chrome, with their
real profile, their extensions, their saved passwords, and their existing
logins.

Every difference between those two environments becomes a defect discovered at
replay time, in production, on a customer's workflow — cookie banners that
appear for the worker and not the operator, MFA challenges the real profile had
already satisfied, popup and download behaviour that differs, selectors that
resolve against a page an extension altered. Recording in the execution
environment makes the recording a *prediction* of the replay rather than a
description of a different session.

This is also why the fidelity argument does not settle it the way it first
appears. A remote browser has full CDP: the accessibility tree
(`Accessibility.getFullAXTree`), DOM snapshots, and the network layer. It can
emit `role` + `name` selectors — exactly the fields `resolveLocator` prefers —
deterministically, with no model inference in the critical path. An extension
content script can read the DOM and compute accessible names, but reaching
network-level detail requires the debugger API and its alarming banner.

### The rest of the comparison, honestly

| | Remote cloud browser | Chrome extension |
|---|---|---|
| Onboarding | No install; but "sign into your ERP inside our browser" is a trust ask | Install + broad host permissions; no login friction |
| Enterprise adoption | Data stays in an environment you control and can describe in a security review | Extension allowlisting is an IT process measured in weeks-to-months at regulated firms |
| Recording fidelity | Full CDP: a11y tree, DOM, network | DOM and a11y; network needs the debugger API |
| Reliability | You own the browser and its version | Chrome release cadence and extension-platform changes are yours to chase |
| Cost | Browser-minutes, and they are not cheap | Near zero |
| Credentials | Ghost holds them — which it already must, to replay | Rides the user's existing session; nothing new held |
| Desktop later | No path to desktop apps | No path to desktop apps |

Cost decided it, and that is a legitimate basis. Browser-minutes are a real
operating line, and an extension gets a working capture path to users in a
fraction of the time.

The credentials row is where the decision has a consequence that must be paid
for explicitly. The extension rides the operator's *existing* session; the
worker has no such session. Recording therefore no longer demonstrates how the
worker will authenticate — the operator was simply already logged in, and that
fact does not travel in the trace. Something has to close that gap, and the
current answer is the wrong one: secrets sit in plaintext in the workflow JSON
(P1-3). Connector credentials with worker-only decryption are the seam between
"recorded here" and "executed there", and the extension decision makes that work
mandatory rather than eventual.

### Execution stays server-side — the extension only records

Stated explicitly because it is the decision that determines what Ghost is.

The extension's responsibilities are: capture browser actions, compute selector
candidates, filter sensitive input, upload a trace. It does **not** execute
workflows.

Execution in the user's browser would ride existing logged-in sessions and dodge
some anti-bot measures, and it would also mean Chrome must stay open, the
laptop must stay awake, and nothing runs overnight. A back-office automation
product whose runs stop when someone closes their laptop is not an operations
platform. Scheduled and unattended execution is most of the value.

This is already how the system is built — the worker owns Chromium and the run
journal — so the decision is really a commitment not to drift, in either
direction: Ghost is a web application, the extension is an optional tool for one
task, and there is no desktop application. The legacy Tauri app at the
repository root is not a future direction.

### Where the extension is straightforwardly better

Applications reachable only inside a corporate network. A Ghost-hosted browser
cannot reach an intranet ERP without networking work; an extension on the
employee's machine already can. Given that "legacy systems behind a login" is
the intended wedge, this is not a minor point in its favour.

### What the extension must therefore be engineered to do

The environment gap is the risk to manage, and it is manageable if capture is
built deliberately rather than as an event log:

- **Emit `role` + `name` selectors at capture time**, computed from the
  accessibility tree in the content script — not inferred later from an opaque
  trace. These are exactly the fields `resolveLocator` prefers, and producing
  them deterministically takes the model out of the correctness path. It then
  does what models are good at: naming steps, proposing where approval gates
  belong, flagging what it could not resolve.
- **Capture several selector candidates per target**, not one. The worker
  replays in a different browser; a single brittle selector is the failure this
  design invites.
- **Record the page URL and a DOM snapshot at each step**, so a replay
  divergence can be diagnosed against what the operator actually saw.
- **Filter at capture, not upload.** Never record password fields, hidden
  inputs, `autocomplete="one-time-code"`, card-shaped input, or session
  cookies. This is the trace hygiene that §1 says cannot be delegated to a
  model's good intentions — and it is much easier to enforce in the content
  script than to strip afterwards.
- **Expect to discover replay-only failures** — cookie banners, MFA prompts the
  operator's profile had already satisfied, popups — and treat each as a gap
  between record and replay environments rather than a one-off bug.

### The longer-term shape

For enterprise accounts that want the intranet case solved properly, the answer
is a **customer-deployed worker** — Ghost's own execution container running
inside the customer's network — which collapses capture and execution back into
one environment. That is the same artifact those accounts will want for
dedicated infrastructure anyway. It is a later concern, not a v1 one.

---

## 4. Repository audit

Ghost is further along than its stage suggests. The run engine — leases, a
hash-chained journal, the restore planner, at-most-once execution across an
approval halt, compensation — is genuinely well built. Org scoping is applied
consistently across all 34 API routes with no missing-authz route. There are no
empty catch blocks. Migrations are clean and sequential. The duplication that
looks like copy-paste (`artifacts` in core and worker, `queue` in three places)
is deliberate layering, and correct.

The gaps are not sloppiness. They are deferrals that stopped being revisited,
plus a small number of real defects. Ordered by business impact.

### P0 — before this is pointed at anything real

| # | Finding | Why it matters | Status |
|---|---|---|---|
| P0-1 | **MFA is bypassable on `/api/agent/*`.** The middleware matcher excludes `/api/agent`, and `resolveAgentPrincipal` accepted a session cookie in addition to a bearer token. `POST /api/agent/runs` starts a real run. | The second factor did not cover the action that moves money. | **Closed.** `resolveAgentPrincipal` now refuses session cookies entirely — bearer credential only (`apps/web/src/lib/agent-auth.ts`). Humans use `/api/runs`; agents mint a key in Settings. |
| P0-2 | **Raw traces shipped to a third party.** | Closed by §1. | Closed |
| P0-3 | **No rate limiting anywhere.** `/api/mfa/verify` accepts unlimited attempts against a six-digit code with a skew window. Also sign-in, `/api/invitations/accept`, `/api/audit/verify`. | An unthrottled TOTP endpoint is not a second factor. | **Closed.** Redis fixed-window limiter (`apps/web/src/lib/rate-limit.ts`) on MFA verify, invite accept, audit verify, and Auth.js GET/POST (`/api/auth/[...nextauth]`). Fail-closed on Redis outage. |

### P1 — before real customers

| # | Finding | Why it matters | Status |
|---|---|---|---|
| P1-1 | `/api/audit/verify` loads the entire org chain with no pagination. | The "prove your audit log is intact" feature is the first thing that breaks at scale, and it takes the web tier with it. | Open — rate-limited; needs checkpointed verify, not just pagination. |
| P1-2 | Screenshots are captured after every step, including `fill` steps marked `sensitive`, and `step-*.png` is on the servable allow-list. OTP and card fields are `type="text"`, so nothing masks them. | Cardholder data at rest in the blob store. | **Partial.** Worker skips screenshot capture for `fill`+`sensitive` (`shouldCaptureScreenshot` in `driver.ts`); editor password-input and secret-reference design still open. |
| P1-3 | Secret values are stored in plaintext in the workflow definition; `sensitive` is a label. There is no secret-reference mechanism. | A database dump is every customer credential. The values also leave via the agent API, which returns `latestVersion.steps` verbatim. | Open — blocked on connector credentials (§5 step 6). |
| P1-4 | `extract` outputs are written into the hash-chained journal in cleartext. | A GDPR erasure request against an intentionally immutable chain is unresolvable. Fix the shape before there is data in it. | Open — needs journal payload allow-list / redaction design. |
| P1-5 | **No RBAC on any business operation.** A `MEMBER` can publish workflows, start runs, approve sensitive steps, and mint agent keys. Separation of duties is enforced (approver ≠ triggerer) but any two colleagues satisfy it. | "Human approval" currently means "anyone with a login." | **Partial.** Minting agent credentials is now OWNER/ADMIN only. Publish / start-run / approve still any member; VIEWER/APPROVER roles not added yet. |
| P1-6 | SSRF: `navigate` accepts any URL, the worker runs `--no-sandbox`, and there is no private-IP denylist. Cloud metadata endpoints are reachable, and `extract` reads the result back out. | Credential theft by any authenticated user. | **Partial.** `checkPublicHttpUrl` blocks cloud metadata + RFC1918 (loopback allowed for fixtures; other private hosts only if they match `APP_URL`). Enforced at schema author time and again in `applyStep`. `--no-sandbox` and DNS-rebinding still open. |
| P1-7 | Three BullMQ workers share one Redis connection, which BullMQ uses for blocking reads. | Intermittent stalls that will look like engine bugs. | **Closed.** Each Worker gets its own Redis connection (`apps/worker/src/index.ts`). |
| P1-8 | No wall-clock run timeout. The lease heartbeat renews unconditionally for as long as the process lives. | Two pathological runs wedge a worker pod for every tenant. | **Closed.** `GHOST_RUN_TIMEOUT_MS` (default 30m); heartbeat stops renewing past the deadline and the loop raises `RUN_TIMEOUT` incident. |
| P1-9 | Concurrency is per-workflow only; the queue is a single global FIFO with no per-org fairness. | One tenant starves all others. No per-tenant SLA is possible. | Open — product/capacity model. |
| P1-10 | `apiCall` and `sendEmail` parse, classify, gate — and execute nothing. The editor refuses them; the API does not. | A human approves "send this invoice," nothing is sent, and the run reports SUCCEEDED. | **Closed.** `authoredSteps` rejects non-`EDITABLE_STEP_TYPES` at the API boundary. |
| P1-11 | Local `pnpm test` exits 0 while skipping ~100 DB-gated tests, including every test of the run/approval/verify loop. | The safety-critical half of the product has no pre-push signal on a developer machine. | **Closed.** `scripts/require-test-env.mjs` refuses `pnpm test` without `DATABASE_URL`/`REDIS_URL`/`GHOST_SESSION_KEY` (escape hatch: `GHOST_ALLOW_SKIP_DB_TESTS=1`). `@ghost/core`'s `test` now depends on its own `build` so prisma generate races are gone. |

### P2 — real, not urgent

Encryption key has no rotation path; unbounded `findMany` on the recordings and
workflows lists; the run-detail SSE issues ~10 queries/second per open page; the
whole journal is re-verified on every job pickup; the compile harness is
find-or-create *by name* on a shared account; `/api/dev/enqueue-noop` ships in
the production bundle behind nothing but a session; `LEASE_MS` is duplicated
across web and worker; agent tokens are hashed with bare SHA-256 (acceptable
given 256-bit random input, but unpeppered); Chromium runs `--no-sandbox`.

Two build-system defects found while running the suite, both in the same family
as P1-11 — green output that covers less than it appears to:

- ~~`turbo.json` omits `GHOST_MFA_KEY` from `globalEnv`~~ — closed; present in root `turbo.json`.
- ~~`test` depends on `^build` but not on a package's own `build`~~ — closed for
  `@ghost/core` (`packages/core/turbo.json` makes `test` depend on `build`).

### Recommended order

1. ~~P0-1 — reject session auth on the agent surface.~~ Done.
2. ~~P0-3 — rate limits on the four named endpoints.~~ Done (incl. Auth.js).
3. P1-5 — finish RBAC on approve / publish / start-run. `isOrgAdmin` already
   exists; minting is gated; the role vocabulary still needs `VIEWER` and
   `APPROVER`.
4. P1-2, P1-3, P1-4 — the secret-handling triad. Screenshot skip for sensitive
   fills is in; secret references, editor masking, journal payload allow-list
   remain.
5. P1-6 remainder (DNS-rebinding / sandbox), P1-1 checkpointed verify.
6. ~~P1-10 — reject unimplemented step types at the API boundary.~~ Done.
7. ~~P1-11 — make local `pnpm test` fail loudly when `DATABASE_URL` is unset.~~ Done.

---

## 5. Sequencing

The failure mode available here is not incapability. It is starting a sixth
subsystem before one complete customer path works end to end — twelve bridges,
each half-way across the river.

The order is therefore deliberately serial, and each step has an acceptance
criterion that can be observed rather than asserted:

| # | Work | Done when |
|---|---|---|
| 1 | **Stabilize.** Merge #403. Fresh-database migration check. Correct the HarnessRouter wording. | `master` holds a working trace → editable workflow path. ✅ |
| 2 | **Compiler boundary.** Extract `WorkflowCompiler`; HarnessRouter becomes one adapter. | Removing the adapter disables compile and breaks nothing else. A test proves the app builds and runs with no `HR_API_KEY`. ✅ |
| 3 | **Capture.** Chrome extension: auth and org binding, event capture, sensitive-field filtering, trace upload. | Record → demonstrate → stop → review → publish, with no manual JSON upload. |
| 4 | **Editor.** The typed step editor covers the whole step model, with validation before publish. | A non-technical operator can correct a generated workflow without touching JSON. |
| 5 | **One production execution path.** Not ten executor types — one reliable browser workflow, proven under failure. | The test below passes. |
| 6 | **Credential boundary.** Org-scoped connector credentials, worker-only decryption, usage audited. | No executor ever receives a credential value from workflow JSON (closes P1-3). |
| 7 | **Reliability layer.** Metering, per-org limits, dead-letter handling, and failure messages that say what changed and whether retry is safe. | — |

### The test that matters — **run, and it passes**

**The workflow must recover without duplicating the mutating action.** Two
drivers now exercise this against a real worker process, a real queue and real
Chromium — not the vitest suite, which calls `runWorkflowJob` in-process and so
has no process to kill, no lease to expire and no BullMQ redelivery.

**`e2e-drive.ts` — the happy path.** Navigate, fill, halt at the gate with the
session captured, approve, resume, restore browser state, submit, verify,
`SUCCEEDED`. Twelve journal entries; `step.succeeded` for the submit step
appears **exactly once**; a duplicate enqueue with the same job id collapses.

**`crash-drive.ts` — the adversarial path.** Spawns its own worker, waits until
the journal shows `step.started` for the submit step with no outcome — the
click is genuinely in flight — then `SIGKILL`s the worker's process group and
starts a fresh one. Observed:

```text
worker exit: code=null signal=SIGKILL
final status: INCIDENT
  OUTCOME_UNKNOWN: step 2 started but never recorded an outcome,
  and its effect cannot safely be repeated
times the submit click succeeded: 0
```

The recovering worker **did not re-click Submit**. `step.started` is recorded
before the effect precisely so a crash leaves the step visibly in flight, and
the state machine then refuses to guess whether it landed — it raises
`step.outcome_unknown` and quarantines the run for a human. For a payment step
that is the only defensible behaviour: an engine that retried here would place
the order twice, and one that assumed success would report a payment that never
happened.

A note on how nearly this was missed. The first version of `crash-drive.ts`
spawned the worker via `npx tsx` and called `child.kill()`, which signals the
*wrapper* — the node process running the worker is a grandchild and survived,
finished the step normally, and the driver printed a confident `PASS` having
tested nothing at all. The driver now spawns with `detached: true`, kills the
process group, and **verifies the process actually died** before drawing any
conclusion. A crash test that cannot prove it caused a crash is worse than no
crash test, because it is believed.

Still unproven, and worth doing next: retry of a failed step, rejecting an
approval, cancelling a run mid-flight, and changing a selector underneath a
published workflow.

### What not to build yet

SSO (§2), a connector catalogue, a template library, agent-to-agent anything, a
marketplace, or a desktop application. Not because they are wrong — several are
on the path — but because each one is a bridge, and none of them is the river.

## 6. What these decisions imply commercially

The decisions above point the same direction: **Ghost owns its runtime.** No
vendor in the execution path, no vendor in the login path, and execution on
Ghost's infrastructure rather than the user's machine. Ghost is a web
application; the extension is an optional tool for one task; there is no
desktop application, and the legacy Tauri tree is not a future direction.

That is more work than the alternatives and it is the only version that survives
a security review — and the only one where a workflow still runs at 3am with the
customer's laptop shut.

The audit's P1 list is close to a restatement of what a buyer's questionnaire
asks: who can approve, what happens to secrets, can you revoke access, can you
prove what ran. P1-5 in particular is not a code-quality item — an approval gate
that any employee can satisfy is not a governance feature, and governance is the
thing being sold.

Two things worth stating plainly against the temptation to build outward:

**The engine is not finished, and it is the product.** `apiCall` and `sendEmail`
are declared and inert (P1-10); there is no capture (§3); connectors exist as
schema with nothing reading them. Adding a connector catalogue, a template
library, or a visual builder on top of an engine that silently no-ops an
approved send makes the failure worse, not better — it multiplies the surface
that can report success without acting.

**Per-execution pricing needs per-execution accounting, and none exists.** There
is no metering of browser minutes, no per-org run quota, no cost attribution
(P1-9 is the same gap seen from the scheduling side). Charging by work rather
than seats is the right instinct for this product, and it is a metering feature
before it is a pricing page.

The narrowest honest description of what Ghost does today: *it replays a
hand-authored browser workflow, stops for human approval before anything
irreversible, verifies the outcome, and leaves a tamper-evident log.* Everything
in this document is either protecting that sentence or extending it.

### Ready for a beta customer when one person can do all of this

```text
create an organization → install the extension → record a real workflow
  → edit the generated steps → connect credentials → test safely → publish
  → run it repeatedly → approve the sensitive actions → diagnose a failure
  → inspect the evidence and audit history
```

Not before. Every item in that line is either built, listed in §5, or listed in
§4 — nothing else is required to get there, and nothing else should be started
until it works.

The wedge that loop serves: **repetitive back-office browser work spanning a
legacy system and an approval.** Three templates on the same architecture —
customer-record update, invoice or payment approval, daily reconciliation —
demonstrate the same five strengths (browser execution, approval gates,
evidence, auditability, recovery) rather than three unrelated capabilities.
"AI automation platform" is a phrase that has been worn smooth; "we replace the
person who logs into the portal and clicks the same six things" is a purchase
order.
