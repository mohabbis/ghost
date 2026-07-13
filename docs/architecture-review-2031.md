# Ghost Architecture Review: Trusted Execution to 2031

**Status:** design review (docs only). No implementation changes implied by this document.  
**Audience:** principal engineers and product owners deciding what Ghost becomes over five years.  
**Relationship to other docs:** This review is the architectural north star where long-horizon ambition conflicts with wedge discipline. The product contract in `AGENTS.md` still governs near-term build order. Ambition wording in [`next-generation-architecture.md`](next-generation-architecture.md) defers to this document on KEEP / REFACTOR / REPLACE / REMOVE and platform ownership.

**Revision note:** Incorporates a second-pass critique that reframes Ghost as desktop *infrastructure* (a trusted execution kernel), not merely a desktop automation application.

---

## North star

> Ghost is not a desktop automation application. Ghost is a **trusted execution kernel** for desktop operating systems. Every interface, planner, workflow, plugin, or AI client must compile intent into a verified Action IR over a typed **Resource Graph** before the kernel deterministically executes it under explicit policy, producing cryptographically verifiable audit and recovery artifacts.

That shifts Ghost from “a really good automation tool” into infrastructure that humans, AI clients, and other automation systems depend on — a position much harder to copy than another assistant with a polished UI.

---

## Verdict

Ghost’s trust pipeline is the correctly chosen *ethical* architecture. The implementation is unevenly mature. The first-pass review correctly identified a missing unify layer (Action IR), but that is not the deepest abstraction.

**Deepest missing abstraction:** a typed **Resource Graph** (what exists).  
**Operating surface over that graph:** a sealed **Action IR** (what to do).  
**Institutional form:** a **Ghost Kernel** that owns Resource Graph, Capability Registry, Policy, Verification, Action IR, Execution Runtime, Audit, and Undo. Everything else is an adapter.

The architecture with the highest probability of still being the best trusted desktop execution platform in 2031:

```text
Ghost Kernel (Rust)
  ├── Resource Graph
  ├── Capability Registry
  ├── Policy Engine
  ├── Verification Engine   ← centerpiece
  ├── Action IR
  ├── Execution Runtime
  ├── Audit + Undo + Snapshots
  └── Event Bus + Scheduler (plans only)

Adapters (never peers of the kernel)
  → Desktop UI / CLI / Voice / App Intents / MCP / Plugins / Shortcuts

Platform products (versioned independently)
  → Ghost Agent macOS · Ghost Agent Windows · Ghost Agent Linux (CI)

Shell
  → thin Tauri (or successor) desktop UI

Planners (behind a Planner Interface — AI disappears architecturally)
  → rules engine · human · OpenAI · Claude · Gemini · MLX · Ollama · …
```

Philosophy refined:

```text
Intent arrives as an event.
Ghost maps reality onto a Resource Graph.
A planner proposes Action IR over that graph.
Ghost verifies assumptions against reality.
Policy applies trust levels.
The user approves.
The kernel executes via named capabilities only.
Ghost verifies again.
Audit and recovery artifacts are sealed.
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
| Doc drift | Guard vs policy naming; wedge docs disagree (Organizer-first vs replay-first); next-gen doc outruns `AGENTS.md` build order |
| What does *not* exist yet | Ghost Kernel boundary, Resource Graph, Capability Registry as product surface, Scheduler-as-planner, trust-level ladder, deterministic state snapshots, Planner Interface abstraction |

Today’s Organizer path is the strongest approximation of kernel behavior: server-side re-plan, policy on actions, undo-before-mutate, sealed audit. The architectural work is to stop treating Organizer / Routines / MCP as peer products and make them **clients of one kernel**.

---

## Deliverable 1 — Subsystem verdicts

For every major subsystem: **KEEP**, **REFACTOR**, **REPLACE**, or **REMOVE**.

| Subsystem | Verdict | Why |
|---|---|---|
| **Rust** | **KEEP** | Kernel language. Policy, graph, IR, FS mutation, crypto, audit survive five years without GC drama at approval boundaries. |
| **Tauri 2** | **KEEP** (shell / adapter only) | Fine as a thin desktop adapter. The kernel must not live in the WebView. |
| **Frontend (vanilla JS)** | **REFACTOR** | Adapter hygiene. Split `main.js`; never rewrite the kernel into React. |
| **Project structure** | **REFACTOR** → kernel layout | `policy/`, `organizer/`, `audit/`, `mcp/` are seeds. Refactor toward kernel crates/modules; quarantine enterprise scaffolding until a playbook exists. |
| **Command architecture** | **KEEP** (+ tighten) | Module + risk class remains. All mutating IPC must enter via kernel events / intents — no bypass. |
| **Security / trust model** | **KEEP** (+ expand) | Deny-by-default and plan-hash tokens stay. Binary approval becomes a **Trust Level** ladder. |
| **Policy engine** | **KEEP** (+ graph-aware) | Pure `evaluate` is excellent. Bind decisions to Resource Graph nodes and trust levels. |
| **Planner(s)** | **REFACTOR** | Organizer planner becomes one producer of Action IR via the **Planner Interface**. AI is not special-cased in architecture. |
| **Audit** | **KEEP** | Hash-chained, export-masked PII. Required kernel output for every mutating run. |
| **Undo** | **KEEP** + **extend with snapshots** | Action-reverse journals stay. Add optional deterministic state snapshots for verification and stronger recovery. |
| **Replay / Routines** | **REFACTOR** | Capture / compression / resolution remain. They become event → intent → IR clients of the kernel, not a parallel product. |
| **MCP** | **KEEP** (narrow adapter) | One AI *ingress* adapter into the kernel — not a second execution engine. No shell tools. |
| **Plugins** | **REPLACE** (from scratch) | Emit **Intent** only. Compose **Capability Registry** entries. Never implement FS/UI ops. Never emit Action IR directly. |
| **Platform backends** | **REFACTOR** → **Agent products** | Promote to versioned **Ghost Agent macOS / Windows / Linux** (Linux = CI), not “Swift helper” footnotes. |
| **Cross-platform** | **KEEP** product focus | Primary: macOS + Windows. Linux agent for CI / headless. Do not pretend headless equals product parity. |

---

## Deliverable 2 — Ideal architecture: the Ghost Kernel

```text
                    ┌─────────────────────────────────────┐
                    │           Ghost Kernel              │
                    │                                     │
 Events ───────────►│  Event Bus                          │
 (FS, UI, MCP,      │       │                             │
  clipboard,        │       ▼                             │
  intents, …)       │  Intent Normalizer                  │
                    │       │                             │
 Scheduler ────────►│       │   (scheduler NEVER executes)│
 (watchers emit     │       ▼                             │
  intents / plan    │  Resource Graph  ◄── Platform Agents│
  requests only)    │       │                             │
                    │       ▼                             │
                    │  Planner Interface                  │
                    │   rules | human | any LLM adapter   │
                    │       │                             │
                    │       ▼                             │
                    │  Action IR  (edges over the graph)  │
                    │       │                             │
                    │       ▼                             │
                    │  Verification Engine  (pre)          │
                    │       │                             │
                    │       ▼                             │
                    │  Policy Engine × Trust Levels       │
                    │       │                             │
                    │       ▼                             │
                    │  User Approval → Signed Token       │
                    │       │                             │
                    │       ▼                             │
                    │  Execution Runtime                  │
                    │   (Capability Registry only)        │
                    │       │                             │
                    │       ▼                             │
                    │  Verification Engine  (post)        │
                    │       │                             │
                    │       ▼                             │
                    │  Audit Chain · Undo · Snapshots     │
                    └─────────────────────────────────────┘
                                      ▲
         ┌────────────┬───────────────┼───────────────┬────────────┐
         │            │               │               │            │
    Desktop UI      MCP           Plugins        App Intents     CLI / Voice
    (adapter)    (adapter)   (intent adapter)     (adapter)      (adapters)
```

### Kernel owns (non-negotiable)

| Kernel component | Role |
|---|---|
| **Resource Graph** | Typed nodes: Desktop, Applications, Windows, Controls, Files, Folders, Clipboard, Secrets, Network, Permissions, Zones. Reality Ghost can name. |
| **Capability Registry** | Named ops the kernel alone implements: `Filesystem.Move`, `UI.Click`, `OCR.Read`, … |
| **Policy Engine** | Decisions over graph nodes × capabilities × trust levels |
| **Verification Engine** | Prove assumptions match reality (pre- and post-execution) — the moat |
| **Action IR** | Sealed, hashable edges over the Resource Graph |
| **Execution Runtime** | Walks IR via Capability Registry only |
| **Audit + Undo + Snapshots** | Cryptographically verifiable history and recovery |
| **Event Bus + Scheduler** | Everything enters as events; scheduler produces plan *requests*, never mutations |

### Adapters (clients of the kernel — not peers)

Desktop UI, CLI, Voice, App Intents, Shortcuts, MCP, Plugins, marketing demos — all compile to **events / intents**. Organizer and Routines are **product surfaces** on the kernel, not separate architectures.

### What must not exist as peer products of the kernel

Chat UI, autonomous loops, knowledge bases, IDEs, email clients, browser-agent cores, plugin-owned executors.

---

## Deliverable 2b — Resource Graph (deeper than Action IR)

Action IR answers **what to do**. The Resource Graph answers **what exists**.

```text
Desktop
├── Applications
│   ├── Finder
│   ├── Outlook
│   └── Excel
├── Windows
│   ├── Downloads
│   └── Workbook
├── Controls
│   ├── Button
│   ├── Table
│   └── TextField
├── Files / Folders
├── Clipboard
├── Secrets
├── Network
├── Permissions
└── Zones
```

Everything is a **node**. Every approved action is an **edge** between nodes.

Instead of reasoning “Move file,” Ghost reasons:

```text
Downloads/file.pdf  ──Filesystem.Move──►  Documents/Invoices/
```

That graph enables permissions, policies, undo, visualization, explanations, plugin composition, and AI (or rules) planning — because every client shares one ontology of reality.

**Action IR operates on the Resource Graph. It does not stand alone.**

First-pass review error corrected: treating Action IR as *the* missing layer under-specified the world model. IR without a graph is a command list. Graph without IR is a inventory. Kernel needs both; the graph is deeper.

---

## Deliverable 2c — Verification as centerpiece

First-pass review still orbited execution. Flip the center of gravity:

```text
Intent
  → Plan (Action IR over Resource Graph)
  → Verify (pre)      ← prove reality matches assumptions
  → Policy × Trust Level
  → Approve
  → Execute
  → Verify (post)     ← prove outcomes match the sealed plan
  → Audit + recovery artifacts
```

Ghost’s moat is not “can click buttons.” The moat is **obsessive proof that reality matched the assumptions under which the user approved.** Automation without verification is a macro recorder. Verification without automation is a viewer. The kernel is both, with verification at the center.

Verification Engine responsibilities (elevate from ad-hoc executor checks):

- Node identity / TOCTOU (file identity, window identity, control identity)
- Canonicalize and boundary re-check
- Precondition proofs from the Resource Graph
- Dry-run feasibility where possible
- Conflict and irreversible-op detection
- Post-condition proofs and snapshot diffs
- Failure that refuses to mutate or seals a partial run honestly

---

## Deliverable 2d — Events, Scheduler, Trust Levels

### Everything is an event

Ghost currently thinks too often in *workflows*. It should think in **events** that may produce intents:

| Event families (examples) | Downstream |
|---|---|
| FilesystemChanged, FolderHashChanged | Planner request |
| Mouse / Keyboard / Clipboard | Capture / compression → intent |
| MCP tool invocation | Intent |
| Plugin intent emission | Intent |
| Shortcut / App Intent / Voice | Intent |
| USBInserted, ScreenUnlocked, AppOpened, UserIdle, CalendarStarts | Scheduler → plan *request* only |

Same pipeline after the event bus. No privileged silent path.

### Scheduler (plans only)

A true scheduler — not “cron for execute”:

```text
When Downloads changes
When Outlook receives attachment
When USB inserted
When screen unlocked
When calendar event starts
When user idle
When folder hash changes
When app opens
```

**Hard rule:** Schedulers **never** execute. They emit events / intents that request plans. Execution remains Verify → Policy → Approve → Execute → Verify. Continuous mutation culture (Hazel-without-preview) is explicitly refused.

### Trust Level system

Binary approve/deny is too coarse. Kernel trust levels:

| Level | Name | Default powers |
|---|---|---|
| **0** | Read only | Graph inspect, safe metadata |
| **1** | Preview only | Plans, dry-runs, diffs — no mutation |
| **2** | Reversible mutation | Moves / renames with undo + snapshots allowed |
| **3** | Irreversible mutation | Deletes / overwrites / sends — explicit, rare |
| **4** | Admin | Zone / policy pack changes, agent install |
| **5** | Developer | Experimental surfaces, unsigned plugin install (still no AI self-approve) |

Policies become simpler: bind capability × resource node class → minimum trust level, then require user elevation / approval tokens for that level. Sticky god-mode remains forbidden.

---

## Deliverable 3 — Platform architecture

**Chosen path:** Rust kernel + versioned **Ghost Agent** products (Option B/C hybrid). Never Option D (SwiftUI-only app rewrite).

| | macOS | Windows | Linux |
|---|---|---|---|
| Role | Primary product | Primary product | CI / headless agent |
| UI adapter | Tauri WebView (near-term) | Tauri WebView | tests only |
| Platform product | **Ghost Agent macOS** (versioned) | **Ghost Agent Windows** (versioned) | **Ghost Agent Linux** (CI) |
| Native depth | Swift / ObjC + XPC: AX, Vision, App Intents | Real UI Automation + secure input; C# agent if Rust UIA stays painful | headless constraints documented |

Think **Docker Engine**, not “Swift helper”: agents version independently, speak a stable protocol to the kernel, and are replaceable without rewriting policy / IR / audit.

### Language ownership

| Concern | Owner | Why |
|---|---|---|
| Kernel (graph, IR, policy, verify, execute, audit) | **Rust** | One trust core |
| UI adapter | JS in Tauri (near-term) | Not the moat |
| Ghost Agent (accessibility, OCR, input inject) | Platform-native (Swift / UIA / possibly C#) | OS depth ceilings |
| Filesystem mutations | **Rust kernel via Capability Registry** | Never LLM, plugin, or agent-authored FS logic outside registered caps |
| AI / planners | **Planner Interface** adapters | Kernel consumes plans; does not know if the planner was a model |
| Networking | Rust / explicit grants | Same envelope as vault / identity |
| Persistence | Rust + redb (**KEEP**) | Current truth |
| MCP / CLI / Voice | Thin adapters into Event Bus | Never bypass kernel |

---

## Deliverable 4 — Product boundaries (NEVER)

Ghost must **never**:

1. Execute arbitrary shell, scripts, or model-generated code.
2. Let AI, plugins, or schedulers mutate files / OS input directly.
3. Bypass or auto-grant approval (no AI self-approval, no sticky trust-level 5 without ceremony).
4. Silently observe (always-on screen / email / browser / mic / camera).
5. Become a general chatbot or second-brain / memory product.
6. Become a browser-agent company (DOM automation as core).
7. Own user cloud storage for workflows / Organizer data by default.
8. Ship plugins that implement capabilities or emit Action IR.
9. Treat Delete / Overwrite as silent defaults (Trust Level 3+ only).
10. Claim production / enterprise compliance for Guard Desk / POS simulations.
11. Grow Linux into a third consumer OS before macOS + Windows kernel + agent parity is boring.
12. Add features that do not strengthen Event → Intent → Graph → IR → Verify → Policy → Approve → Execute → Verify → Audit.
13. Allow a Scheduler to call Execution Runtime.
14. Special-case “AI” as a privileged peer of the kernel.

Strong boundaries *are* the category definition.

---

## Deliverable 5 — Missing layers (corrected stack)

Ordered by depth:

### 1. Ghost Kernel (institutional form)

The largest first-pass omission. Organizer, Routines, MCP, Plugins, App Intents must become **clients**, not peer architectures. Without a kernel boundary, feature accumulation recreates five trust paths.

### 2. Resource Graph (deepest world model)

What exists. Nodes for apps, windows, controls, files, clipboard, secrets, network, permissions, zones. Enables policy, undo, explanation, plugins, and planning.

### 3. Capability Registry (execution vocabulary)

Ghost owns `Filesystem.Move`, `Filesystem.Copy`, `Filesystem.Delete`, `UI.Click`, `UI.Type`, `OCR.Read`, `Clipboard.Copy`, …. Plugins *compose* capabilities; they never *implement* them. This matters more than a Plugin API.

### 4. Action IR (sealed edges over the graph)

Typed, hash-stable, reviewable, trust-annotated. Only approveable mutation artifact. Executable without trusting the client. Journalable.

### 5. Verification Engine (centerpiece behavior)

Pre/post proof against the Resource Graph and optional snapshots. Moat vs macro recorders and chat agents.

### Why this beats “add more AI”

AI (and every other planner) becomes an adapter behind the Planner Interface. Graph + capabilities + verify + trust levels remain valuable if models vanish tomorrow and become *more* valuable if models improve — because denser proposals still must compile onto a shared reality model and survive proof.

---

## Deliverable 6 — Technology review

No fence-sitting.

| Tech | Decision | Why |
|---|---|---|
| Swift | **Adopt** inside **Ghost Agent macOS** | Versioned agent, not temp scripts |
| SwiftUI | **Avoid** (app rewrite) | Wrong bet; UI is an adapter |
| App Intents | **Experiment** (adapter → Event Bus) | Native entry; kernel still approves |
| XPC | **Adopt** | Isolate agent privileges from kernel process where useful |
| ScreenCaptureKit | **Avoid** as default | No-hidden-capture posture |
| Accessibility API | **Adopt** (deepen in Agent) | Resource Graph nodes for UI |
| Vision | **Adopt** (via Agent) | OCR nodes / capabilities |
| CoreML / MLX | **Avoid** in kernel; **Experiment** as Planner Interface backends | Suggestion only |
| WinUI | **Avoid** near-term | Tauri adapter sufficient |
| Windows UI Automation | **Adopt** in **Ghost Agent Windows** | Real control graph nodes |
| C# | **Experiment** for Windows Agent if needed | Agent product, not kernel rewrite |
| React / Solid / Svelte | **Avoid** | Adapter rewrite tax |
| TypeScript | **Experiment** (UI adapter contracts) | Not a strategy rewrite |
| SQLite | **Avoid** return | redb stays |
| SQLCipher | **Experiment** under enterprise threat only | — |
| WASM Component Model | **Adopt** for plugins | Intent/composition only; capability-bounded |
| gRPC | **Experiment** for Agent↔Kernel if local ABI needs it; else keep simple | Don’t overbuild early |
| JSON-RPC | **Adopt** / keep | MCP + possibly agent protocol |
| Cap’n Proto / FlatBuffers | **Avoid** near-term | Versioned serde IR + graph schema first |
| Rhai / Lua / Python | **Avoid** as extension runtimes | Script execution is the anti-product |
| Go / Zig | **Avoid** | No second kernel language |

---

## Deliverable 7 — Plugins and the Capability Registry

**Plugins: yes, eventually** — after Resource Graph, Capability Registry, and verification-first IR exist.

### Correct composition

```text
Wrong (first-pass):   Plugin ──► Action IR
Right:                Plugin ──► Intent ──► Planner Interface ──► Action IR
                      Plugin composes Capability Registry entries; never implements them
```

| Dimension | Rule |
|---|---|
| Form | WASM Component Model |
| Emits | **Intent** (+ optional planning hints) — never Action IR, never capabilities |
| Composes | Capability Registry names declared in manifest |
| Forbidden | FS/UI/network implementation, approve, scheduler execute, native `.dylib`/`.dll`, Python executors |
| Permissions | Per Zone / resource-node scopes; user-granted |
| Limits | CPU / memory / time sandbox |
| Trust | Signed packages; unsigned → Trust Level 5 developer path only |
| Updates | Explicit; no silent hot-patch of kernel or capabilities |

**Capability Registry > Plugin API.** If the registry is right, plugins are small. If plugins must invent Move/Delete, the kernel has already failed.

---

## Deliverable 8 — AI disappears architecturally

First-pass sketch still had a privileged `AI → Planner` edge. Replace it:

```text
Planner Interface
  ├── Rules engine
  ├── Human (manual plan authoring / edits)
  ├── OpenAI / Claude / Gemini / …
  ├── MLX / Ollama / local OpenAI-compatible
  └── Future planners…
         │
         ▼
   Action IR over Resource Graph
         │
         ▼
   Verification Engine (does not care who proposed)
```

**If LLMs become 100× better:** denser intents and plans; kernel value rises because proof, trust levels, and recovery remain scarce. Do not thin approval.

**If LLMs disappear tomorrow:** rules engine + human planners + Organizer/Routines still work. MCP goes quiet. Kernel remains desktop infrastructure.

Design law:

```text
Planner quality scales proposal density.
Kernel quality scales refusal and proof quality.
Ghost must not know or care whether a planner is AI.
```

---

## Deliverable 9 — Competitive analysis

| Product | Copy | Refuse |
|---|---|---|
| Apple Shortcuts | Intent surfaces, App Intents, clear permission prompts | Closed ecosystem; no portable kernel thesis |
| Keyboard Maestro | Macro reliability | Power without verify / audit defaults |
| Hazel | Folder watch elegance | Silent continuous *execution* — steal triggers, not auto-mutate |
| Power Automate Desktop | Connector gravity (later, via capabilities) | Cloud-first RPA sprawl |
| Raycast AI | Fast launcher UX | Chat / omnibox identity |
| Open Interpreter | — | Shell-from-LLM |
| Claude Desktop | MCP client habit | Chat-as-product |
| Cursor | Review feedback UX | Competing as IDE |
| Mycroft / Open Voice OS | — | Voice-assistant identity |
| OpenAI Operator / Browser Use / Stagehand | Targeting lessons | Browser-agent category |
| Microsoft Copilot | OS integration ambition | Ambient OS assistant persona |
| OpenAdapt | Recording lineage | Unbounded observe / automate |
| Docker / OS kernels (analogy only) | Versioned engine + thin clients | Literally becoming an OS |

Steal **trigger ergonomics** and **permission clarity**. Refuse ambient autonomy. Position against all of them as **infrastructure they cannot safely replace**.

---

## Deliverable 10 — Three-year roadmap (risk reduction)

Platform evolution that reduces architectural risk — not feature accumulation.

```text
Y1 Kernel seed + parity
  → Declare kernel module boundary in-tree (even before perfect purity)
  → Resource Graph v0 (files/folders/zones first; UI nodes stubbed)
  → Capability Registry v0 (lift Organizer FS ops into named caps)
  → Action IR over that graph (extract from PlanAction)
  → Routines become event→intent→IR clients; app/window Zones
  → Trust Level ladder replaces binary approval UX incrementally
  → Modularize main.js (adapter hygiene)
  → Release signing honesty (Authenticode)

Y2 Verification + Agents + Planner Interface
  → Verification Engine first-class (pre/post); optional state snapshots
  → Scheduler v0: watchers emit plan requests only
  → Ghost Agent macOS / Windows as versioned products
  → Planner Interface; AI adapters are not special
  → MCP and UI speak events/intents only; remote MCP stays experimental until boring
  → Universal approval tokens bound to IR hash + trust level

Y3 Extensibility without surrender
  → WASM plugins emit Intent; compose Capability Registry
  → Snapshot-assisted recovery matures
  → Multi-device only via user-hosted relay patterns — never silent cloud execution
  → Enterprise policy packs as kernel config, not new products
```

Each year closes a risk class: peer-product sprawl → proof gaps → agent depth → plugin attack surface.

---

## Deliverable 11 — Critical review

Assume the current architecture is wrong enough to hurt.

1. **Peer-product sprawl without a kernel.** Organizer, Routines, MCP, experimental AI, and future plugins each invent trust. Marketing will speak as if they are equal; they are not.
2. **Command lists without a world model.** Organizer `PlanAction` is strong but File-shaped. Without a Resource Graph, UI automation and FS automation never share ontology.
3. **Execution-centered storytelling.** Docs and UI celebrate “Approve & Organize / Replay.” The scarce skill is verification. Center that in architecture and UX.
4. **Binary approval.** Everything interesting lives between “read” and “irreversible.” Lack of trust levels forces either nag fatigue or silent stretch of permissions.
5. **Workflow orthodoxy.** Thinking only in saved workflows under-models FS watchers, App Intents, MCP, and clipboard. Event-first is the durable ingress.
6. **Plugin IR temptation.** Letting plugins emit Action IR makes them mini-executors. Intent-only is the firewall.
7. **Capability leakage.** If plugins or agents implement Move/Click themselves, the Capability Registry is theater.
8. **“Swift helper” under-ambition.** Ephemeral scripts and Win32 point-lookup are not Agent products. Depth debt compounds.
9. **Undo without snapshots.** Reverse journals fail when the world moves; verification without snapshots is weaker than it must be.
10. **AI as architectural celebrity.** Hard-wiring AI→Planner brands Ghost as an assistant. Planner Interface keeps Ghost infrastructure even when the cleverest planner is a human checklist.
11. **`main.js` monolith + enterprise scaffolding.** Adapter bloat and roadmap cosplay both distract from kernel extraction.
12. **Scheduler danger.** The day a watcher calls execute directly, Ghost becomes Hazel-with-AI and exits the category.

---

## Deterministic state snapshots

Complement action-reverse undo:

```text
Snapshot₀  →  Execute (capability-bound)  →  Snapshot₁  →  Compare / Verify
```

Snapshots improve post-verification, explainability, and recovery when reverse ops are insufficient (partial runs, external interference, irreversible edges denied after the fact). They do not replace journals; they reinforce proof.

Privacy default: snapshot scope is minimized to Resource Graph nodes touched by the sealed plan — never whole-disk ambient imaging.

---

## Final recommendation

Build Ghost as a **trusted execution kernel for desktop operating systems**:

- typed **Resource Graph** as the world model;
- **Capability Registry** as the only mutation vocabulary;
- **Action IR** as sealed edges over that graph;
- **Verification** (pre and post) as the centerpiece;
- **Trust Levels** instead of binary permission theater;
- **Events + Scheduler** that never execute;
- **Planner Interface** so AI disappears as a special case;
- **Ghost Agents** as versioned platform products;
- every UI, MCP client, plugin, and App Intent as an **adapter**.

Do not become an assistant. Do not become a shell. Do not become Zapier. Do not become Hazel-with-chat.

Outlast models and fashion by owning **reality, refusal, proof, and recovery**.

---

## Reading order

1. `AGENTS.md` — product contract and near-term build order  
2. This document — five-year kernel north star  
3. [`trust-pipeline.md`](trust-pipeline.md) — stage definitions (reinterpret with Verify-first and Trust Levels)  
4. [`core-boundaries.md`](core-boundaries.md) — stable vs experimental  
5. [`next-generation-architecture.md`](next-generation-architecture.md) — MCP / marketplace ambition (defers here on conflicts)  
6. [`full-repo-audit-2026-07-13.md`](full-repo-audit-2026-07-13.md) — as-built gaps  
