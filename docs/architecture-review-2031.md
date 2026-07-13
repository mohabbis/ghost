# Ghost Architecture Review: Trusted Execution to 2031

**Status:** design review (docs only). No implementation changes implied by this document.  
**Audience:** principal engineers and product owners deciding what Ghost becomes over five years.  
**Relationship to other docs:** This review is the architectural north star where long-horizon ambition conflicts with wedge discipline. The product contract in `AGENTS.md` still governs near-term build order. Ambition wording in [`next-generation-architecture.md`](next-generation-architecture.md) defers to this document on KEEP / REFACTOR / REPLACE / REMOVE and platform ownership.

---

## Verdict

Ghost’s trust pipeline is the correctly chosen product architecture. The implementation is unevenly mature. The missing layer is a single **Action IR** that every surface (Organizer, Routines, MCP, plugins) must compile into before policy, approval, and execution.

The architecture with the highest probability of still being the best trusted desktop execution platform in 2031:

```text
Rust-owned Action IR + policy + verification + execution + audit + undo
  → thin Tauri (or successor) desktop shell
  → platform-native agents (Swift on macOS; stronger UIA on Windows)
  → MCP as the only AI ingress
  → WASM planning-only plugins
  → no chat product, no silent agent, no shell-from-LLM
```

Philosophy (already in the repo — keep it):

```text
The model reasons. Ghost verifies. The user approves. Deterministic code executes.
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

---

## Deliverable 1 — Subsystem verdicts

For every major subsystem: **KEEP**, **REFACTOR**, **REPLACE**, or **REMOVE**.

| Subsystem | Verdict | Why |
|---|---|---|
| **Rust** | **KEEP** | Correct ownership language for policy, IR, FS mutation, crypto, audit. Survives five years; no GC pauses at the approval boundary. |
| **Tauri 2** | **KEEP** (shell only) | Right for a small team shipping macOS + Windows. Do not bet the moat on WebView. Thin shell forever. |
| **Frontend (vanilla JS)** | **REFACTOR** | Correct for now; `main.js` is a maintainability trap. Split by view / module. Do **not** rewrite to React as a strategy project. |
| **Project structure** | **REFACTOR** | `policy/`, `organizer/`, `audit/`, `mcp/` are right. Cull or quarantine commandless enterprise scaffolds until a playbook exists. Unify Routines onto the same IR as Organizer. |
| **Command architecture** | **KEEP** (+ tighten) | Module + risk class + registry is sound. Forbid new commands that bypass Plan → Policy → Approve. |
| **Security / trust model** | **KEEP** | Deny-by-default, re-plan on execute, plan-hash tokens — this *is* the product. |
| **Policy engine** | **KEEP** (+ extend) | Pure `evaluate` is excellent. Must grow to cover OS capabilities with real app / window Zones (not just folders). |
| **Planner** | **KEEP** (Organizer); **REFACTOR** (unify) | Organizer planner is the template. Replay / MCP must emit the same Action IR. |
| **Audit** | **KEEP** | Hash-chained, export-masked PII is correct. Extend to all mutating runtimes. |
| **Undo** | **KEEP** (Organizer); **REFACTOR** (Routines) | Undo-before-mutate + WAL is gold. Replay undo is too weak (typed backspace only). |
| **Replay** | **REFACTOR** | Keep capture / compression / resolution; force through Action IR + policy Zones; stop growing as a parallel product. |
| **MCP direction** | **KEEP** (narrow) | One ingress for AI clients. Organizer-shaped tools only until IR is universal. Refuse shell / filesystem free-for-all tools. |
| **Plugin direction** | **REPLACE** (design from scratch when ready) | No SDK today. Future: WASM planners only — never native executors. |
| **Cross-platform** | **KEEP** strategy; **REFACTOR** backends | Product = macOS + Windows. Linux = CI. Deepen native agents; stop pretending headless equals product parity. |

Nothing in this table is **REMOVE** of a shipping trust primitive. What should be removed over time is scaffolding and ambition that does not strengthen the pipeline (see Deliverable 11).

---

## Deliverable 2 — Ideal architecture

```text
Desktop UI · MCP clients · Platform Intents
                 │
                 ▼
          Intent Normalizer
                 │
     ┌───────────┴───────────┐
     ▼                       ▼
Deterministic Planners   AI Suggest-Only
     │                       │
     └───────────┬───────────┘
                 ▼
        Sealed Action Plan (IR)
                 │
                 ▼
           Policy Engine
                 │
                 ▼
        Verification Engine
                 │
                 ▼
          User Approval
                 │
                 ▼
      Signed Approval Token
                 │
                 ▼
         Execution Runtime
                 │
                 ▼
          Platform Agents
                 │
         ┌───────┴───────┐
         ▼               ▼
     Audit Chain    Undo Journal
         │               │
         └───────┬───────┘
                 ▼
       Local Persistence
```

### What must exist

1. **Intent Normalizer** — maps UI clicks, MCP tools, and later App Intents into a structured intent (never directly into filesystem ops).
2. **Deterministic Planners** — Organizer planner, Routine compressor → planner, filing preview. AI may feed *suggestions* only.
3. **Action IR (sealed plan)** — the only approveable artifact: typed ops, targets, risks, conflicts, undo class, plan hash.
4. **Policy Engine** — Zones, capabilities, trust levels (exists; extend).
5. **Verification Engine** — distinct from policy: TOCTOU identity, canonicalize, dry-run feasibility, conflict re-check, irreversible-op detection. Pieces already live inside the Organizer executor; elevate them to a layer.
6. **User Approval + signed token** — exists for MCP; make universal for all mutating runs.
7. **Execution Runtime** — walks IR only: no AI, no plugins, no frontend-trusted plans. (`organizer_execute` already re-plans server-side — generalize that law.)
8. **Platform Agents** — thin OS adapters: FS, AX / UIA, input inject, OCR.
9. **Audit + Undo + Persistence** — exist; one journal model for all runtimes.

### What must not exist as peer products

Chat UI, autonomous loops, knowledge bases, IDEs, email clients, browser-agent cores.

---

## Deliverable 3 — Platform architecture

**Chosen path:** Option B now (Rust + Swift helpers), Option C when Windows automation depth demands a C# agent — **never** Option D (native SwiftUI-only).

| | macOS | Windows | Linux |
|---|---|---|---|
| Role | Primary product | Primary product | CI / headless only |
| UI shell | Tauri WebView (KEEP near-term) | Tauri WebView | headless tests |
| Native deepening | Swift helper for AX / Vision / App Intents / XPC | Proper UI Automation + secure input path; WinUI only if a shell rewrite is justified | none |

### Language ownership

| Concern | Owner | Why |
|---|---|---|
| UI | JS in Tauri (near-term); optional later native shells | Not the moat; shipping velocity beats widget purity |
| Automation / input | Rust orchestration + platform agent | Must stay interruptible and auditable |
| Accessibility | Platform agent (Swift / ObjC on macOS; UIA COM on Windows) | Today’s AX wrappers in Rust are a depth ceiling — deepen natively |
| Filesystem | **Rust only** | Cross-OS identity, TOCTOU, undo — never hand to an LLM or plugin |
| Workflow runtime | **Rust** | Determinism and a single IR |
| Policy engine | **Rust** | Pure, testable, deny-by-default |
| AI integration | Rust adapters + MCP; models never execute | Survive model churn |
| Networking | Rust (explicit grant only) | Same envelope as vault / identity |
| Persistence | Rust + redb (**KEEP**) | Already migrated; avoid SQLCipher rewriting unless an enterprise threat requires it |
| MCP | Rust binary (`ghost mcp serve`) | Correct placement; one protocol |

**Why not full native SwiftUI:** abandons Windows and the “universal execution layer for any AI client” thesis.

**Why not “mostly Rust forever with zero native code”:** Accessibility, App Intents, and Vision quality ceilings. Apple and Microsoft ship OS surfaces that require first-party languages for *integration* credibility (Shortcuts / Power Automate parity of integration — not of product shape).

---

## Deliverable 4 — Product boundaries (NEVER)

Ghost must **never**:

1. Execute arbitrary shell, scripts, or model-generated code.
2. Let AI mutate files, send messages, or drive OS input directly.
3. Bypass or auto-grant approval (no AI self-approval, no sticky god-mode).
4. Silently observe (always-on screen / email / browser / mic / camera).
5. Become a general chatbot or second-brain / memory product.
6. Become a browser-agent company (DOM automation as core).
7. Own user cloud storage for workflows / Organizer data by default.
8. Ship marketplace plugins that can execute.
9. Treat Delete / Overwrite as silent defaults.
10. Claim production / enterprise compliance for Guard Desk / POS simulations.
11. Grow Linux into a third product OS before macOS + Windows trust parity is boring.
12. Add features that do not strengthen Intent → Plan → Policy → Approve → Execute → Audit → Undo.

Strong boundaries *are* the category definition. Feature accumulation that weakens refusal quality is a product failure even if the demo looks impressive.

---

## Deliverable 5 — The missing layer: Action IR

**Name:** Action Intermediate Representation — a sealed, hashable, reviewable plan document.

This is a **layer**, not a feature. It matters more than adding AI because every new surface without a shared IR invents a parallel trust path.

| Today | Problem |
|---|---|
| Organizer `PlanAction` | Strong, but Organizer-shaped only |
| Routines `CompressedStep` / `InputEvent` | Parallel world; weak undo / policy |
| MCP tools | Regenerate Organizer plans; not a universal contract |
| Plugins (future) | No typed target to compile into |

Without one IR, every new surface invents a new plan shape → feature accumulation and trust gaps (audit finding RPL-001: replay not Organizer-grade).

### Action IR requirements

- Typed ops (`Move`, `Rename`, `Click`, `Type`, `Wait`, …) with risk class.
- Policy-annotated before UI shows it.
- Hash-stable for approval tokens.
- Executable without re-trusting the client.
- Journalable for undo class.
- Explainable in plain language.

Verification Engine then becomes “prove this IR is still safe to run,” not a pile of ad-hoc checks inside each executor.

---

## Deliverable 6 — Technology review

No fence-sitting.

| Tech | Decision | Why |
|---|---|---|
| Swift | **Adopt** (macOS agent / OCR / App Intents host) | Replace temp Swift scripts with a versioned helper |
| SwiftUI | **Avoid** (as app rewrite) | Wrong bet for a cross-platform execution product |
| App Intents | **Experiment** | Native “ask Ghost to organize X” entry — Ghost still approves |
| XPC | **Adopt** (macOS agent isolation) | Privilege separation for AX / OCR |
| ScreenCaptureKit | **Avoid** as default | Conflicts with no-hidden-capture; opt-in only if ever needed |
| Accessibility API | **Adopt** (deepen) | Core of Routines target identity |
| Vision | **Adopt** (via helper) | Already used for OCR |
| CoreML / MLX | **Avoid** in core | Suggestion side-channel only if ever; not on the trust path |
| WinUI | **Avoid** near-term | Tauri shell is sufficient |
| Windows UI Automation | **Adopt** | Current Win32 point-lookup is under-powered vs claim |
| React / Solid / Svelte | **Avoid** | Rewrite tax without moat; modularize vanilla JS |
| TypeScript | **Experiment** (gradual JSDoc / TS for modules) | Optional hardening of UI contracts — not a rewrite |
| SQLite | **Avoid** (do not return) | redb is current truth |
| SQLCipher | **Experiment** only under enterprise threat | Not a default rewrite |
| WASM Component Model | **Adopt** for plugins when built | Capability-bounded planners |
| gRPC | **Avoid** | Overkill for a local product; MCP / JSON-RPC fits |
| JSON-RPC | **Adopt** / keep | MCP already |
| Cap’n Proto / FlatBuffers | **Avoid** | Premature for local plans; use versioned serde IR |
| Rhai / Lua / Python | **Avoid** as extension runtimes | Script execution is the anti-product |
| Go / Zig | **Avoid** | No second systems language |
| C# | **Experiment** later | Windows agent if UIA-in-Rust stays painful |

---

## Deliverable 7 — Plugins

**Yes, eventually** — but not as a product priority before Action IR and Routines policy parity exist.

### Runtime design (from scratch)

| Dimension | Rule |
|---|---|
| Form | WASM Component Model (planning plugins only) |
| Not allowed | Native `.dylib` / `.dll` plugins, subprocess executors, Python |
| Powers | Propose Action IR fragments / classifiers / naming |
| Forbidden powers | FS mutate, OS input, network, approve, read arbitrary files unless capability grant |
| Permissions | Declared capabilities; user grants per Zone |
| Limits | CPU / memory / time; no threads escaping the sandbox |
| Versioning | Semver + Ghost IR version negotiation |
| Trust | Signed packages; default deny unsigned |
| Updates | Explicit user update; no silent hot-patch of the trust path |

Plugins feed **planners**, never the **Execution Runtime**. If a plugin cannot express its output as Action IR, it does not ship.

There is no Ghost plugin SDK today. Do not assume Tauri plugins (`opener`, `dialog`, `updater`) are a product plugin model — they are shell infrastructure only.

---

## Deliverable 8 — AI extremes

**If LLMs become 100× better:** Ghost becomes *more* valuable. Better planners propose denser Action IR; the scarce resource remains trustworthy local mutation. Expand verification, conflict detection, semantic targets, and explainability. Do **not** thin out approval.

**If LLMs disappear tomorrow:** Organizer, Zones, policy, audit, undo, and deterministic Routines still work. MCP goes idle. The product remains a trusted local organizer and routine runner. That is the anti-fragility test — pass it, or the category is fake.

Design law:

```text
Model quality scales proposal quality.
Ghost quality scales refusal quality.
```

Ghost must become more valuable as models improve, not less. Any design that makes approval optional when proposals get “good enough” destroys the category.

---

## Deliverable 9 — Competitive analysis

| Product | Copy | Refuse |
|---|---|---|
| Apple Shortcuts | Intent surfaces, App Intents entry, clear permission prompts | Closed ecosystem lock-in; weak cross-app trust audit story as moat |
| Keyboard Maestro | Power-user reliability of macros | Raw power without preview / audit as default |
| Hazel | Folder-rule simplicity | Silent continuous mutation culture |
| Power Automate Desktop | Enterprise connector gravity (later) | Cloud-first, sprawling RPA complexity |
| Raycast AI | Fast launcher UX patterns | Becoming a launcher / chat omnibox |
| Open Interpreter | — | Shell-from-LLM |
| Claude Desktop | MCP client habit | Chat-as-product |
| Cursor | Tight feedback UX for review | Competing as an IDE |
| Mycroft / Open Voice OS | — | Voice-assistant identity |
| OpenAI Operator / Browser Use / Stagehand | Careful targeting lessons | Browser-agent category |
| Microsoft Copilot | OS integration ambition | Ambient OS assistant persona |
| OpenAdapt | Recording lineage awareness | Unbounded observe / automate |

Ghost’s category is not “another assistant.” It is **trusted local execution**. Copy interface discipline and permission clarity. Refuse ambient autonomy and shell/DOM freestyle.

---

## Deliverable 10 — Three-year roadmap (risk reduction)

Not feature accumulation — **platform evolution**. Each phase removes a class of architectural risk.

```text
Y1 Foundation + Parity
  → Freeze new product surfaces
  → Action IR extraction from Organizer PlanAction
  → Routines compile to Action IR + app/window Zones + real undo classes
  → Cull or strictly quarantine enterprise scaffolding
  → Modularize main.js; keep vanilla
  → Release signing honesty (Authenticode gap closed)

Y2 Verification + Ingress
  → Verification Engine as a first-class crate / layer
  → Universal approval tokens for all mutations
  → MCP tools speak Action IR only; remote MCP stays experimental until boring
  → macOS Swift / XPC agent for AX / OCR / App Intents experiment
  → Windows UIA agent upgrade

Y3 Extensibility without surrender
  → WASM planning plugins (signed, capability-bounded)
  → Optional enterprise policy packs / audit export maturity
  → Multi-device only via user-hosted relay patterns already sketched
    — never silent cloud execution
```

Risks closed in order: dual runtimes → IR dualism → native quality ceiling → plugin attack surface. Not “broader category.”

---

## Deliverable 11 — Critical review

Assume the current architecture is wrong enough to hurt. Do not protect existing code simply because it exists.

1. **Two products in one process.** Organizer is trust-grade; Routines are best-effort. Markets and bugs punish the weaker path while marketing speaks as if both are equal.
2. **`main.js` monolith.** ~5.8k LOC with weak module boundaries slows iteration and invites experimental bleed into default UX.
3. **Platform depth theater.** Windows “UIA” commentary vs Win32 point APIs; macOS OCR via ephemeral Swift scripts — not ship-grade agents.
4. **Premature enterprise folders.** Commandless finance / fraud / compliance code reads as roadmap cosplay and distracts from IR / parity.
5. **Doc / ambition inflation.** Next-gen “OS for safe AI actions” vs `AGENTS.md` wedge discipline vs replay-first product roadmap — strategy thrash invites feature accumulation.
6. **MCP remote surface before IR universalization.** LAN / TLS / relay expands attack surface while the plan model is still Organizer-shaped.
7. **Tauri WebView forever risk.** Fine as a shell; fatal if trusted logic creeps into JS. Re-plan-on-execute is the vaccine — keep enforcing it.
8. **Scale failure mode.** Not QPS — *policy complexity × OS flaky targets × human approval fatigue*. Without IR, verification, and progressive trust levels, users will demand “just auto-approve,” which kills the category.
9. **Rewrite risk.** Full SwiftUI or React remakes would burn years and not improve Action IR. The dangerous rewrite is “agent framework” addiction.
10. **Complexity trap.** Every experimental feature behind a flag still has maintenance tax (CI experimental leg, docs drift, marketing leaks).

---

## Final recommendation

Build Ghost as the sealed **Action IR + policy + verification + approval + deterministic runtime** for local desktop work, exposed to humans via a thin desktop UI and to models via MCP — with platform-native agents for accessibility quality, and WASM planners only when the IR is law.

Do not become an assistant. Do not become a shell. Do not become Zapier.

Outlast the models by owning **refusal, proof, and undo**.

---

## Reading order

1. `AGENTS.md` — product contract and near-term build order  
2. This document — five-year architecture north star  
3. [`trust-pipeline.md`](trust-pipeline.md) — stage definitions  
4. [`core-boundaries.md`](core-boundaries.md) — stable vs experimental  
5. [`next-generation-architecture.md`](next-generation-architecture.md) — MCP / marketplace ambition (defers here on conflicts)  
6. [`full-repo-audit-2026-07-13.md`](full-repo-audit-2026-07-13.md) — as-built gaps  
