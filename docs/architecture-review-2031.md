# Ghost Architecture Review: Trusted Execution to 2031

**Status:** design review (docs only). No implementation changes implied by this document.  
**Audience:** principal engineers and product owners deciding what Ghost becomes over five years.  
**Relationship to other docs:** This review is the architectural north star where long-horizon ambition conflicts with wedge discipline. The product contract in `AGENTS.md` still governs near-term build order. Ambition wording in [`next-generation-architecture.md`](next-generation-architecture.md) defers to this document on category, KEEP / REFACTOR / REPLACE / REMOVE, and ownership boundaries.

**Revision history:**  
1. First pass — trust pipeline + Action IR as missing layer.  
2. Second pass — Ghost Kernel over a typed Resource Graph; verification-first.  
3. **This pass** — Kernel and graph remain components. The stronger category is a **transaction system for human-computer work**.

---

## North star

> Ghost is a **local transaction authority for human-computer work**. It converts intent into bounded, version-aware, proof-carrying transactions over typed desktop resources. It authorizes exact capabilities, executes deterministically through platform agents, verifies observed effects, compensates partial failure, and issues signed receipts. AI, humans, schedulers, and plugins are merely sources of intent. None of them receive execution authority.

That is beyond an assistant, beyond automation, and beyond “kernel” as an implementation metaphor.

Ghost is an **ACID-like trust layer for desktop actions**, adapted to a world where perfect atomicity is impossible — so it provides explicit consistency, isolation, durability, compensation, and evidence instead. Fifty years of desktop software omitted this layer. That omission is the opportunity.

```text
Declare intent
  → Resolve resources
  → Construct transaction
  → Validate invariants
  → Authorize exact effects
  → Execute bounded steps
  → Verify postconditions
  → Commit or compensate
  → Issue receipt
```

The user approves a **transaction**, not a vague workflow.

---

## Verdict

| Framing | Role |
|---|---|
| “Desktop automation” | Undersells; competitors copy UIs and macros |
| “Trusted execution kernel” | Useful *implementation* shape; still undersells the category |
| **Transaction authority** | Defensible product category — typed effects, leases, proof, compensation, receipts |

Components from prior passes remain necessary and are **subordinate** to the transaction:

| Component | Role under the transaction thesis |
|---|---|
| Resource Graph | Typed, versioned world model (read/write sets live here) |
| Capability Registry | Mutation vocabulary the execution plane alone implements |
| Action IR | Sealed steps inside a transaction — often proof-carrying |
| Verification Authority | Preconditions / postconditions / uncertainty gates |
| Ghost Agents | Platform data-plane drivers |
| Planner Interface | Produces assumptions + proposed transactions; never executes |
| Event log | Immutable triggers; never execute |

Philosophy refined:

```text
Intent arrives as an event.
Ghost resolves versioned resources.
A planner proposes a transaction (assumptions made explicit).
Verification proves feasibility against observed versions.
Policy + approval issue a capability lease and execution token.
The local execution plane runs bounded, idempotent steps.
Ghost verifies postconditions, commits or compensates, and signs a receipt.
No remote party, model, plugin, or UI holds execution authority.
```

---

## Ground truth (as built)

Snapshot grounded in the tree at review time (v1.2.9 era):

| Fact | Evidence |
|---|---|
| Version / stack | Tauri 2, Rust 2021, vanilla JS frontend (no bundler) |
| Strong path | Organizer: plan → policy → approve → execute → audit / undo / WAL / seal |
| Partial path | Routines / replay: compression + Guard + one-shot approve; not Organizer-grade Zones / per-step policy |
| MCP | Local stdio Organizer tools + signed approval tokens **built**; HTTP / TLS / relay experimental |
| Plugins | **None** (docs only) |
| Hotspots | `src/main.js` (~5.8k LOC); `src-tauri/src/engine.rs` (~1.6k); `src-tauri/src/platform/macos.rs` (~1.5k) |
| Scaffolding debt | `enterprise/`, `finance/`, `fraud/`, `checks/`, `compliance/`, `data_protection/` — commandless models, no IPC |
| Closest existing approx. of a transaction | Organizer re-plan on execute, per-action policy, undo-before-mutate, sealed audit chain, plan-hash MCP tokens |
| Not yet present | First-class `Transaction`, control/execution plane split, proof obligations on actions, resource version identity, concurrency leases, sagas / compensation classes, signed receipts as product surface, assumption objects, multi-axis uncertainty, execution classes A–D, event-sourced scheduler, idempotency keys, Ghost Execution Protocol, declarative policy language (Cedar / constrained Rego) |

Organizer is the best seed of transaction semantics in the repo. The architectural work is to stop treating Organizer / Routines / MCP as peer products and grow them into **clients of one transaction authority**.

---

## Deliverable 1 — Subsystem verdicts

| Subsystem | Verdict | Why |
|---|---|---|
| **Rust** | **KEEP** | Language of the transaction authority and execution plane. |
| **Tauri 2** | **KEEP** (UI adapter only) | Instrument panel shell. Trust logic must not live in the WebView. |
| **Frontend** | **REFACTOR** | Become a **transaction inspector** (assumptions, authority, evidence, recovery) — not a Zapier canvas. Modularize `main.js`. |
| **Project structure** | **REFACTOR** | Evolve toward control plane / execution plane; Organizer remains the first transaction compiler for FS. |
| **Command architecture** | **KEEP** (+ tighten) | Mutating IPC enters only as intent → transaction; no bypass. |
| **Security / trust** | **KEEP** (+ decompose) | Split **permission / capability / lease / approval / execution token** — not one checkbox. |
| **Policy engine** | **KEEP** → grow into Policy Authority; later **Cedar** (or constrained Rego), not cute YAML. |
| **Planner(s)** | **REFACTOR** | Emit **assumptions + proposed Transaction**; hide behind Planner Interface. |
| **Audit** | **REFACTOR** → **signed receipts** | Passive logs become verifiable transaction receipts. |
| **Undo** | **KEEP** + **sagas** | Exact reverse where possible; compensation plans where not; classify reversibility. |
| **Replay / Routines** | **REFACTOR** | Event → intent → transaction candidates; same authority as Organizer. |
| **MCP** | **KEEP** (ingress adapter) | One client of the future **Ghost Execution Protocol** — not the internal architecture. |
| **Plugins** | **REPLACE** | Emit intent / compose capabilities; never implement ops or emit bare Action IR as execution authority. |
| **Platform backends** | **REFACTOR** → versioned **Ghost Agents** in the **execution data plane**. |
| **Cross-platform** | **KEEP** | macOS + Windows product; Linux agent for CI. |

---

## Deliverable 2 — Ideal architecture: transaction authority

```text
                         Clients
     UI / MCP / CLI / App Intents / Schedulers / Plugins
                            |
                            v
                     Intent Gateway
                            |
                            v
                  Transaction Compiler
           assumptions / resources / action plan
                            |
                            v
                Resource & State Authority
          identity / versions / leases / snapshots
                            |
                            v
                    Policy Authority
            permissions / capabilities / constraints
                            |
                            v
                  Verification Authority
       preconditions / feasibility / uncertainty / proof
                            |
                            v
                    Approval Authority
       scoped consent / capability lease / signed token
                            |
                            v
                Local Execution Data Plane
        deterministic steps / checkpoints / idempotency
                            |
                 +----------+----------+
                 |                     |
                 v                     v
           Platform Agents      Transaction Journal
        macOS / Windows          events / compensation
                 |                     |
                 +----------+----------+
                            v
                 Postcondition Verification
                            |
                            v
                  Commit or Compensate
                            |
                            v
                   Signed Execution Receipt
```

### Control plane vs execution plane

**Essential split.**

| Control plane owns | Execution plane owns |
|---|---|
| Intent interpretation | Deterministic actions |
| Planning / planner selection | OS interaction via Agents |
| Plugin composition | Snapshots |
| Policy evaluation | Verification probes |
| Approval / lease issuance | Journaling / idempotency |
| Scheduling / transaction creation | Interruption / rollback / compensate |
| Resource discovery (orchestration) | Audit receipt sealing |
| | **Must work with no model, no network, no plugin runtime, no UI** |

```text
Control Plane
    ↓ signed transaction + execution token
Execution Plane
    ↓ evidence + result
Control Plane
```

Remote control, MCP, enterprise policy, and multiple UIs become safer **because** the local execution plane retains final authority.

### The central object: Transaction

Not a workflow, not an action list, not a chat reply:

```text
Transaction {
  id
  intent
  assumptions[]
  reads: ResourceSet
  writes: ResourceSet
  actions[]                  // often proof-carrying
  invariants[]
  preconditions[]
  postconditions[]
  compensation_plan
  approval_scope
  evidence_requirements[]
  execution_class            // A | B | C | D
  idempotency_key
}
```

Desktop concepts almost never exposed today — Ghost must own them: read set, write set, affected resources, invariants, commit criteria, compensation, evidence, exact authorization scope.

---

## Deliverable 2b — Proof-carrying actions

Not formal-methods certificates on day one — **proof obligations**.

```text
action: filesystem.move
subject:
  resource_id: file:8c19...
  expected_hash: a73f...
destination:
  resource_id: folder:91ab...
requires:
  - source.exists == true
  - source.hash == a73f...
  - destination.contains_name("invoice.pdf") == false
  - capability.filesystem_move == granted
ensures:
  - source.exists == false
  - destination/invoice.pdf.exists == true
  - destination/invoice.pdf.hash == a73f...
```

The executor **rejects** actions whose obligations cannot be established.

| Authority | Question |
|---|---|
| Policy | *May* this happen? |
| Verification | Is the world in the state required for this to happen *safely*? |

Both are mandatory. Policy alone is theater.

---

## Deliverable 2c — Resource identity, versions, concurrency

A graph of “Finder, window, button, file” is insufficient without **identity vs appearance**.

Every resource carries at least:

| Field | Purpose |
|---|---|
| Logical identity | Stable Ghost id (`file:8c19`, `ui:excel:button:export`) |
| Observed identity | Path, AX attrs, geometry, window instance |
| Version | Content hash, inode + volume, UI generation |
| Provenance | How observed |
| Confidence | Target-identity confidence (see uncertainty axes) |
| Freshness | `observed_at` |
| Authority | Which Agent / plane observed it |

**Concurrency control (major prior omission):** desktop state races Ghost while it plans.

Required equivalents of database controls:

- optimistic locking / resource version checks  
- leases  
- stale-plan detection  
- conflict detection  
- transaction invalidation  
- retry **only after re-planning**

```text
Plan hash valid
AND resource version changed
⇒ approval invalidated
```

A plan approved against version 12 must not execute silently against version 14. This principle outranks model intelligence.

---

## Deliverable 2d — Authority stack: permission → lease → token

Binary approval is too primitive. Distinct objects:

| Object | Meaning |
|---|---|
| **Permission** | Broad OS grant (Accessibility, etc.) |
| **Capability** | What kind of operation exists in the registry |
| **Lease** | Temporary bounded authority over resources + constraints |
| **Approval** | User consent for one transaction or class |
| **Execution token** | Cryptographic artifact consumed by the execution plane |

Example lease:

```text
capability: filesystem.move
resources: [zone:downloads, zone:documents/invoices]
constraints:
  max_actions: 25
  overwrite: false
  delete: false
  same_volume_only: true
expires_at: ...
transaction_hash: sha256:...
```

This raises convenience without collapsing into “always allow everything” — the inevitable response to approval fatigue.

Trust Levels 0–5 from the prior pass remain a UX ladder over this stack; they do not replace leases/tokens.

---

## Deliverable 2e — Sagas, execution classes, snapshots

### Sagas (not pretend-everything-is-reversible)

Desktop ops cross systems. Some steps cannot undo.

Every action declares:

```text
reversibility: exact | compensatable | best_effort | irreversible
```

That classification drives approval and evidence requirements. On partial failure the transaction presents a **human recovery plan**, marks partial commit honestly, and runs the compensation plan (never silently invents external undo such as “retract the email”).

### Execution classes (better than fuzzy low/medium/high)

| Class | Examples | Mandates |
|---|---|---|
| **A** Pure read | scan, classify, summarize | No mutation |
| **B** Locally reversible | rename, move, mkdir | Exact undo + version checks |
| **C** Compensatable | upload, draft, setting change | Compensation plan + stronger evidence |
| **D** Irreversible / socially consequential | send email, submit, publish, permanent delete, money | Per-action or elevated approval; forbid unattended when verification is weak |

### Snapshots

```text
Snapshot₀ → Execute (idempotent steps) → Snapshot₁ → Compare / Verify
```

Scope minimized to the transaction write set — never ambient whole-disk imaging.

---

## Deliverable 2f — Assumptions, uncertainty, events, idempotency

### Assumptions as first-class objects

Planners must emit:

```text
assumptions:
  - id: a1
    statement: "…"
    confidence: 0.88
    validation: user_confirmed | unresolved | machine_checked
```

Transactions with unresolved **high-impact** assumptions do not reach the execution plane. Explanations stop being decorative.

### Multi-axis uncertainty (not one ceremonial score)

| Axis | Example policy gate |
|---|---|
| Intent confidence | Low → clarify before compile |
| Target identity confidence | `< 0.85` → manual target confirmation |
| Expected outcome confidence | Low → tighten evidence requirements |
| Verification strength | `visual_only` → forbid unattended |
| Recovery strength | `irreversible` → Class D / per-action approval |

### Event sourcing for triggers

Immutable events: `FolderChanged`, `MailReceived`, `UserRequested`, `TimerElapsed`, `AppOpened`, `MCPRequestReceived`, …

```text
Event → Intent candidate → Transaction proposal
```

Enables replay, debugging, deduplication, idempotency, causality, auditability, and “why did Ghost propose this?”  
**Triggers never execute.**

### Idempotency designed in

Keys: `transaction-id + action-id + resource-version`.

Runtime states (must not collapse uncertainty):

```text
not_started | started | effect_observed | verified
| committed | compensated | failed | unknown
```

`unknown` is first-class. Lying by forcing success/failure is how recovery dies.

---

## Deliverable 3 — Platform architecture

**Local execution data plane** + versioned **Ghost Agents** (macOS / Windows / Linux-CI). UI remains a thin adapter.

| Concern | Owner |
|---|---|
| Transaction authority (compile, policy, verify, approve, journal, receipt) | **Rust control + execution core** |
| OS mutation / AX / OCR / input | **Ghost Agent** products |
| UI | Transaction instrument panel (Tauri/JS near-term) |
| AI / rules / humans | Planner Interface → proposed transactions only |
| MCP / App Intents / CLI / Voice | Intent Gateway adapters |
| Remote clients | May submit intent, inspect, approve leases — **never own execution** |

Remote “execute X” means: local plane checks token, policy, resource versions, local conditions — then proceeds or refuses.

---

## Deliverable 4 — Product boundaries (NEVER)

Ghost must **never**:

1. Give execution authority to AI, plugins, schedulers, or remote clients.  
2. Execute shell / model-generated code.  
3. Collapses permission / capability / lease / approval / token into one checkbox.  
4. Execute against stale resource versions after approval.  
5. Pretend irreversible steps are undoable.  
6. Let triggers or watchers call the execution plane.  
7. Treat MCP as the internal architecture (it is an ingress).  
8. Ship ambient observation (camera / mic / always-on screen / silent email read).  
9. Become a chatbot, IDE, knowledge base, or browser-agent company.  
10. Center the product on a visual workflow canvas instead of transaction review.  
11. Invent an ad-hoc authorization YAML dialect when Cedar (or constrained Rego) exists.  
12. Collapse `unknown` runtime state into fake success/failure.  
13. Claim Organizer-grade trust for surfaces that lack version checks, proof obligations, and receipts.  
14. Weaken verification because “models got better.”

---

## Deliverable 5 — Missing layers (stack, deepest first)

1. **Transaction authority** — category and central object.  
2. **Control / execution plane split** — safety boundary.  
3. **Resource & State Authority** — identity, versions, freshness, concurrency.  
4. **Proof-carrying Action IR** — obligations, not vibes.  
5. **Capability leases + execution tokens** — exact authority.  
6. **Verification Authority** — pre and post; uncertainty-gated.  
7. **Saga / compensation model** — honesty about irreversibility.  
8. **Signed receipts** — durable proof for humans and enterprises.  
9. **Ghost Execution Protocol** — public, versioned envelopes (Intent, Resource Descriptor, Action IR, Policy Decision, Approval Lease, Execution Result, Evidence, Receipt). MCP becomes one client of this protocol.  
10. **Declarative policy language** (Cedar preferred) — when hard-coded Rust policies saturate.

Kernel / Resource Graph / Capability Registry from the prior pass are **how** you build this — not the product category.

**Moat (not AI, not polish):**

```text
Typed resources
+ Versioned state
+ Proof-carrying actions
+ Capability leases
+ Transaction semantics
+ Verification evidence
+ Compensation
+ Signed receipts
```

Competitors copy UIs, prompts, MCP tools, and Swift glue. They struggle to copy years of failure handling under this trust model.

---

## Deliverable 6 — Technology review

| Tech | Decision | Why |
|---|---|---|
| Cedar | **Adopt** (policy language direction) | Resource / principal / action / context fit; avoid DIY YAML auth |
| Rego | **Experiment** only under strict constraints | Powerful; easy to create unsound policy sprawl |
| Swift / XPC / AX / Vision | **Adopt** in Ghost Agent macOS | Execution data plane |
| Windows UI Automation / possible C# | **Adopt / Experiment** in Ghost Agent Windows | Control identity |
| SwiftUI / WinUI app rewrite | **Avoid** | UI is instrument panel, not category |
| ScreenCaptureKit ambient | **Avoid** | Observation boundary |
| WASM plugins | **Adopt** later | Intent + composition only |
| JSON-RPC | **Adopt** | MCP + possibly Agent protocol |
| gRPC | **Experiment** | Agent↔core if needed |
| Cap’n Proto / FlatBuffers | **Avoid** early | Versioned serde protocol first |
| Rhai / Lua / Python executors | **Avoid** | Anti-product |
| React remakes | **Avoid** | Does not create transactions |
| redb | **KEEP** | Persistence; receipts/journals evolve here |
| Ed25519 (or existing envelope crypto) | **Adopt** for receipt / token signatures | Align with vault/auth primitives already in tree |
| Formal theorem provers | **Avoid** near-term | Proof *obligations* first; formal methods later if ever |

---

## Deliverable 7 — Plugins, planners, protocol

| Source | May do | Must not do |
|---|---|---|
| Plugin | Emit intent; compose registered capabilities | Implement FS/UI; emit executable IR as authority; approve |
| Planner (AI or not) | Propose Transaction + assumptions | Execute; self-approve; skip unresolved high-impact assumptions |
| Scheduler | Append events → transaction candidates | Call execution plane |
| MCP / remote | Intent, inspect, approve leases, monitor | Own the machine |
| UI | Transaction review / instrument panel | Be the source of truth for plans (execution plane re-validates) |

**Ghost Execution Protocol** (versioned public envelopes) is the durable integration surface. MCP is not Ghost’s spinal cord.

---

## Deliverable 8 — AI extremes (unchanged law, sharper)

**AI disappears architecturally** behind the Planner Interface. The transaction authority does not know or care whether the planner was Claude, a rules engine, or a human checklist.

**If models are 100× better:** denser intents and better assumption lists — Ghost’s value rises because leases, version checks, proof, and receipts remain scarce. Do not thin approval.

**If models vanish:** Organizer-class transactions, human planning, rules, receipts, and compensation still work.

```text
Planner quality scales proposal density.
Transaction authority quality scales refusal, proof, and recovery.
```

---

## Deliverable 9 — Competitive analysis

| Product | Copy | Refuse |
|---|---|---|
| Apple Shortcuts / App Intents | Intent entry, permission clarity | Closed ecosystem without portable transaction receipts |
| Keyboard Maestro | Reliability of macros | Power without proof / leases |
| Hazel | Event triggers | Auto-execute watchers |
| Power Automate | Connector gravity later | Cloud RPA without local execution authority |
| Zapier-like canvases | — | Workflow-canvas-as-product |
| Open Interpreter / Operator / Browser Use | — | Shell / DOM freestyle authority |
| Claude Desktop / Copilot | Client habits (MCP) | Chat-as-authority |
| Databases / ledger systems (analogy) | Transactions, isolation, durability, receipts | Pretending the desktop is ACID-atomic |

Steal **authorization ergonomics**. Refuse **ambient execution**. Compete as **infrastructure they cannot safely replace**.

---

## Deliverable 10 — Three-year roadmap (risk reduction)

```text
Y1 — Transaction seed (from Organizer)
  → Name control vs execution plane in-tree
  → Transaction object v0 (reads/writes/actions/assumptions/compensation stub)
  → Resource version identity for files (hash/inode) on Organizer path
  → Stale-plan invalidation on version change
  → Execution classes A/B for Organizer; D still denied by default
  → Receipt v0 (hash-chain evolved into signed transaction receipt)
  → Idempotency keys on executor actions
  → UI: transaction review surfaces (assumptions, authority, undo class)
  → Authenticode / signing honesty

Y2 — Proof, leases, agents, concurrency
  → Proof obligations on FS actions; verification authority crate
  → Capability leases + execution tokens (decompose today’s one-shot approve)
  → Event-sourced triggers; scheduler candidates never execute
  → Saga/compensation for Class C; explicit irreversible labeling
  → Ghost Agents versioned; multi-axis uncertainty gates
  → Ghost Execution Protocol draft; MCP speaks it
  → Planner Interface; assumptions required

Y3 — Protocol + policy language + ecosystem
  → Cedar (or constrained Rego) for Policy Authority
  → WASM plugins (intent-only)
  → Remote approve / local execute proven
  → Receipts suitable for enterprise export without becoming cloud-first
  → Class D paths only with mandatory evidence + elevated leases
```

Each year removes a failure class: vague workflows → stale races → weak authority mash → protocol lock-in / policy chaos.

---

## Deliverable 11 — Critical review

1. **Category under-sell.** Shipping “automation with good prompts” invites copycats. Transaction authority is harder and worth it.  
2. **Workflow-as-center.** Workflows hide assumptions, versions, and partial failure. Transactions force them into the open.  
3. **No concurrency model.** The desktop is adversarial mid-plan. Without version checks, approved plans are lies.  
4. **Authority mush.** One approve button ≠ permission ≠ capability ≠ lease ≠ token.  
5. **Undo fantasy.** Pretending uploads and emails reverse cleanly destroys trust at the first saga.  
6. **Audit as log file.** Without signed receipts, “we logged it” is not proof.  
7. **Ceremonial confidence.** One float is not enforceable risk. Multi-axis uncertainty is.  
8. **MCP spine temptation.** External protocol ≠ internal architecture.  
9. **Remote execution creep.** Convenience will demand it; local plane must refuse by design.  
10. **Policy in ad-hoc Rust forever.** Will calcify; plan Cedar, don’t invent YAML.  
11. **Canvas UI distraction.** Instrument panel first — checklist / transaction inspector.  
12. **Idempotency last.** Crash mid-run without keys produces double moves and forged success.  
13. **Unknown state collapsed.** Systems that cannot admit `unknown` cannot recover.  
14. **Two products without one authority.** Organizer vs Routines without shared transaction semantics recreates today’s trust gap.

---

## UI: instrument panel, not workflow canvas

Primary surfaces show:

- What Ghost believes (Resource & State Authority)  
- What it plans (Transaction)  
- Assumptions and their validation state  
- Authority required (lease scope)  
- What could go wrong / reversibility class  
- Evidence to be collected  
- What actually happened (receipt)

Graph editors may exist later as power tools. They are not the product center.

---

## Final recommendation

Build Ghost as the **local transaction authority for human-computer work**:

- typed, versioned resources;  
- proof-carrying actions;  
- capability leases and execution tokens;  
- control plane vs execution plane;  
- verification before and after;  
- sagas and honest compensation;  
- event-sourced triggers that never execute;  
- signed receipts;  
- a public Ghost Execution Protocol;  

with AI, humans, schedulers, and plugins as **intent sources only**.

Do not become an assistant. Do not become Zapier. Do not become a kernel metaphor without transaction semantics.

Outlast fashion by owning **exact authorization, versioned reality, proof, compensation, and receipt**.

---

## Reading order

1. `AGENTS.md` — near-term wedge and non-negotiables  
2. This document — five-year category and architecture  
3. [`trust-pipeline.md`](trust-pipeline.md) — reinterpret stages as transaction phases  
4. [`core-boundaries.md`](core-boundaries.md) — stable vs experimental  
5. [`next-generation-architecture.md`](next-generation-architecture.md) — MCP ambition as **protocol client**  
6. [`full-repo-audit-2026-07-13.md`](full-repo-audit-2026-07-13.md) — as-built gaps  
