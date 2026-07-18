# Ghost Architecture Decision Records

**Status:** Principal engineer design review (2026-07-13).  
**Audience:** Engineers implementing or amending Ghost architecture.  
**Related:** [`architecture-debt-register.md`](architecture-debt-register.md), [`permanent-architecture-invariants.md`](permanent-architecture-invariants.md).

This file captures accepted architectural decisions after the adversarial freeze review. Long-horizon vision documents ([`architecture-review-2031.md`](architecture-review-2031.md), [`financial-transaction-authority.md`](financial-transaction-authority.md)) are **superseded** for engineering priority by these ADRs unless an ADR is explicitly amended with prototype proof.

**Format:** Each ADR includes Status, Decision, Context, Alternatives, Tradeoffs, Consequences, and Future Reconsideration Criteria.

---

### ADR-0001: Ghost is a local transaction authority (restricted scope)

**Status:** Accepted

**Decision:** Ghost is a **restricted local-first transaction authority** for human-computer work. It converts bounded intent into deterministic, authorized, auditable mutations over user-selected resources. It is not an AI assistant, RPA platform, workflow builder, or financial application.

**Context:** Multiple architecture passes positioned Ghost as assistant, kernel, Git-for-AI, and bank-adjacent authority. The implemented wedge — Organizer — proves a narrower, defensible category: trustworthy local execution with evidence of what changed.

**Alternatives:**
- Generic desktop automation (Keyboard Maestro class) — rejected; no trust pipeline.
- Cloud RPA (Power Automate class) — rejected; violates local-first contract.
- Financial operations platform — rejected; no command surface, no SoR connectors.

**Tradeoffs:** Smaller TAM narrative vs. harder-to-copy trust model. Sales must not promise breadth the repo cannot support.

**Consequences:** All new features must map to the seven-stage trust pipeline ([`trust-pipeline.md`](trust-pipeline.md)). Marketing and README must match [`PROJECT_STATE.md`](PROJECT_STATE.md).

**Future Reconsideration:** Only if a signed enterprise contract requires a documented guarantee class **and** a passing prototype exists for that class. Amend via new ADR; do not extend vision docs.

---

### ADR-0002: Executable mutations flow only through deterministic plan + policy + approval

**Status:** Accepted

**Decision:** No code path may mutate user state without: (1) a deterministic plan produced by Ghost backend logic, (2) `policy::evaluate` (or equivalent) per action, and (3) explicit user approval recorded in provenance.

**Context:** Trust pipeline is the moat. Organizer implements this end-to-end. Routines and experimental paths partially bypass it.

**Alternatives:**
- Client-supplied plans — rejected; `organizer_execute` re-plans server-side ([`commands/organizer.rs`](src-tauri/src/commands/organizer.rs)).
- Model output as plan — rejected; AI suggests; deterministic code plans.

**Tradeoffs:** Latency (re-plan on execute) vs. tamper resistance. Extra CPU on every execute vs. stale-plan safety.

**Consequences:** New mutating commands require risk class, policy check, approval gate, audit, and undo where reversible. Register in [`command-registry.md`](command-registry.md).

**Future Reconsideration:** If profiling proves re-plan cost blocks UX at scale, amend with cached plan hash + version binding — not client plan trust.

**Grounding:** [`organizer/executor.rs`](src-tauri/src/organizer/executor.rs), [`policy/engine.rs`](src-tauri/src/policy/engine.rs).

---

### ADR-0003: AI generates intent; never holds execution authority

**Status:** Accepted

**Decision:** AI providers (OpenAI, Anthropic, local LLM, external MCP clients) may propose intent, classifications, labels, and explanations. They may not approve plans, hold leases, dispatch OS actions, or bypass policy.

**Context:** Product non-negotiable #4 in `AGENTS.md`. `intelligence/` and MCP are gated or token-bound.

**Alternatives:**
- Agentic auto-execution — rejected.
- Model-generated shell/scripts — rejected ([`next-generation-architecture.md`](next-generation-architecture.md) principle 1).

**Tradeoffs:** Slower UX (human in loop) vs. safety. Competitive "autonomous agent" narratives must be refused.

**Consequences:** `suggestion_is_safe()` and experimental gates remain mandatory. Intelligence commands stay behind `--features experimental`.

**Future Reconsideration:** Never for execution authority. Suggestion quality may improve; approval requirement does not relax.

**Grounding:** [`intelligence/`](src-tauri/src/intelligence/), [`mcp/approval.rs`](src-tauri/src/mcp/approval.rs).

---

### ADR-0004: Policy evaluation is pure, deny-by-default, Rust-typed (not Cedar)

**Status:** Accepted

**Decision:** Policy remains a pure Rust module (`policy/`) with typed `Capability`, `Zone`, `FolderRule`, and `PolicyDecision`. Deny-by-default. No Cedar, Rego, or ad-hoc YAML policy language in the near term.

**Context:** [`policy-engine.md`](policy-engine.md) describes working code with extensive tests. Cedar appears in long-horizon docs as Y3 direction — premature for current scale.

**Alternatives:**
- Cedar + signed bundles — deferred ([`financial-transaction-authority.md`](financial-transaction-authority.md) §9).
- Hand-grown YAML — rejected.
- Unconstrained OPA/Rego — rejected.

**Tradeoffs:** Rust policy calcifies at very large rule counts vs. no policy-language operational burden today.

**Consequences:** New capabilities extend `policy/capability.rs` and tests. Policy changes ship with code review, not bundle deployment.

**Future Reconsideration:** When rule count and overlay complexity exceed maintainability (debt register: team cannot reason about deny paths in one review), prototype Cedar embed with signed bundles. Requires ADR amendment + migration plan.

**Grounding:** [`policy/`](src-tauri/src/policy/).

---

### ADR-0005: Verification is executor postconditions, not a separate authority plane

**Status:** Accepted

**Decision:** Verification means deterministic postcondition checks in the executor (and replay path): source exists, target absent, hash/inode match, policy re-check. There is no standalone "Verification Authority" crate or control-plane service.

**Context:** [`architecture-review-2031.md`](architecture-review-2031.md) proposed Policy vs Verification split. Organizer already implements both questions in one execution path: policy asks "may this happen?"; executor asks "is the world in the required state?"

**Alternatives:**
- Separate verification crate — rejected as architecture theater until multi-system sagas exist.
- Visual-only verification as proof — rejected for unattended mutation.

**Tradeoffs:** Simpler module graph vs. less formal separation for future cross-system steps.

**Consequences:** Postcondition logic lives in `organizer/executor.rs`, `policy/boundary.rs`, `organizer/file_identity.rs`. MCP "verification" is run metadata summary only.

**Future Reconsideration:** If compensation sagas across heterogeneous systems ship, extract shared verification traits — not a new authority plane without prototype.

**Grounding:** [`organizer/executor.rs`](src-tauri/src/organizer/executor.rs), [`policy/boundary.rs`](src-tauri/src/policy/boundary.rs).

---

### ADR-0006: Authorization is user approval + scoped tokens; not capability leases

**Status:** Accepted

**Decision:** Authorization stack for the product wedge: OS permissions (Accessibility, etc.) + `FolderRule` grants + UI approval + one-shot MCP approval tokens + routine replay fingerprint approval. **Capability leases** (time-bounded multi-action authority) are deleted from active architecture.

**Context:** Long-horizon docs collapsed permission/capability/lease/token/approval — correctly diagnosing mush, but over-engineering the fix. Current tokens + rules suffice for Organizer and MCP.

**Alternatives:**
- Full lease model with expiry and material-change invalidation — deferred.
- M-of-N / SoD enterprise gates — deferred until external IdP.

**Tradeoffs:** More frequent approvals vs. simpler mental model. Approval fatigue handled via `TrustLevel::Automate` on narrow rules, not blanket leases.

**Consequences:** Do not implement lease store. MCP tokens remain plan-hash-bound and single-use.

**Future Reconsideration:** Enterprise customer with IdP integration and documented SoD requirement. Prototype lease on one vertical before generalizing.

**Grounding:** [`mcp/approval.rs`](src-tauri/src/mcp/approval.rs), [`policy/zone.rs`](src-tauri/src/policy/zone.rs), [`commands/core.rs`](src-tauri/src/commands/core.rs) (`approve_routine_replay`).

---

### ADR-0007: Resource binding uses path identity + hash for filesystem; UI binding is best-effort

**Status:** Accepted

**Decision:** Filesystem resource binding uses path containment (`policy` whole-component rules), content hash, and inode/volume identity (`file_identity.rs`) with execution-time re-verify. UI automation binding uses window title, AX/UIA attributes, geometry, and template match — **best-effort**, not proof of business effect.

**Context:** Bank-adjacent docs require universal resource graph. Implemented code binds files well; replay resolution chain handles UI with explicit fallback labeling (`ResolutionKind`).

**Alternatives:**
- Universal Resource Graph with logical IDs — deleted near-term.
- Visual-only identity as proof — rejected for unattended Class C/D ops.

**Tradeoffs:** Honest uncertainty on UI vs. marketing "semantic automation" claims.

**Consequences:** Organizer TOCTOU tests remain mandatory. Replay history must show coordinate fallback share. UI automation never labeled "verified" in audit export.

**Future Reconsideration:** If AX/UIA semantic verification reaches benchmark threshold in `resolution_benchmark.rs`, amend evidence labels in export — not a new graph subsystem.

**Grounding:** [`organizer/file_identity.rs`](src-tauri/src/organizer/file_identity.rs), [`core/replay_support.rs`](src-tauri/src/core/replay_support.rs).

---

### ADR-0008: Stale plans invalidate execution via server-side re-plan

**Status:** Accepted

**Decision:** Mutating execution never trusts a client-supplied plan snapshot. `organizer_execute` accepts Zone identity (and approval context), re-plans server-side, and re-evaluates policy per action at execution time.

**Context:** Deliberate trust choice documented in [`PROJECT_STATE.md`](PROJECT_STATE.md) §3e. Prevents tampered or stale UI plans from reaching filesystem.

**Alternatives:**
- Trust signed plan blob from frontend — rejected.
- Execute cached plan without re-check — rejected.

**Tradeoffs:** Duplicate planner work on execute vs. strong tamper and drift resistance.

**Consequences:** Frontend preview (`organizer_plan`) is indicative; execution plan may differ if disk changed. UI must surface diffs when re-plan changes outcomes.

**Future Reconsideration:** Plan hash + resource version vector may optimize re-plan — must still re-evaluate policy per action.

**Grounding:** [`commands/organizer.rs`](src-tauri/src/commands/organizer.rs), [`organizer/planner.rs`](src-tauri/src/organizer/planner.rs).

---

### ADR-0009: Audit hash chain is receipt v0; universal signed receipt bundle deferred

**Status:** Accepted

**Decision:** Tamper-evident hash-chained execution seals (`storage/executions.rs`, `organizer_verify_audit_chain`) constitute **receipt v0**. A universal signed receipt bundle (transaction.json, plan.ir, policy-decision.json, evidence-manifest.json, etc.) is **deferred**.

**Context:** [`financial-transaction-authority.md`](financial-transaction-authority.md) §12 describes aspirational bundle. Organizer seal + export is implemented and offline-verifiable.

**Alternatives:**
- Full receipt bundle now — rejected; no consumers, high doc burden.
- Unsigned audit log only — rejected; hash chain already shipped.

**Tradeoffs:** Enterprise export format immaturity vs. shipping verifiable local proof today.

**Consequences:** Claims use "tamper-evident audit chain" not "signed transaction receipt bundle." Export adds PII mask; stored chain unredacted (ADR-0019).

**Future Reconsideration:** When enterprise export customers need standard bundle format, add fields incrementally to export — not a parallel receipt subsystem.

**Grounding:** [`storage/executions.rs`](src-tauri/src/storage/executions.rs), [`commands/organizer.rs`](src-tauri/src/commands/organizer.rs).

---

### ADR-0010: Undo-before-mutate is mandatory for reversible local mutations

**Status:** Accepted

**Decision:** For reversible filesystem mutations, undo journal entries are written **before** the mutation occurs. Undo replay runs journal in reverse. Empty-folder removal only; never recursive delete via undo.

**Context:** Trust invariant #8 in product docs. `executor.rs` records `UndoOp` before `fs` calls.

**Alternatives:**
- Post-mutate undo write — rejected; crash loses recovery.
- Compensating saga runtime — merged into undo for FS; no separate engine.

**Tradeoffs:** Extra journal IO per action vs. crash-safe recovery (WAL in `storage/executions.rs`).

**Consequences:** Replay undo remains partial (typed text only) until Phase 2 convergence milestone.

**Future Reconsideration:** Extend undo journal schema for replay steps that are locally reversible — via ADR amendment, not new saga subsystem.

**Grounding:** [`organizer/executor.rs`](src-tauri/src/organizer/executor.rs), [`audit/undo_journal.rs`](src-tauri/src/audit/undo_journal.rs), [`organizer/undo.rs`](src-tauri/src/organizer/undo.rs).

---

### ADR-0011: External systems are never assumed atomic; reconciliation is human/API follow-up

**Status:** Accepted

**Decision:** Ghost does not implement a **Reconciliation Engine** product subsystem. External effects (API timeout, email accept, SaaS submit) are recorded as executed/dispatched with honest outcome labels; reconciliation with systems-of-record is human process or future API follow-up — not an in-app engine with queues and SLA aging.

**Context:** [`financial-transaction-authority.md`](financial-transaction-authority.md) correctly rejects ACID on desktop. `finance/reconciliation/matcher.rs` is a pure read-only helper, not an engine.

**Alternatives:**
- Full reconciliation engine — deleted from active architecture.
- Pretend timeout = success — rejected.

**Tradeoffs:** Honest "unknown/indeterminate" UX vs. automation completeness narrative.

**Consequences:** No reconciliation commands. Power BI / Fabric export pushes audit metadata — not settlement proof.

**Future Reconsideration:** Tier-1 API adapter with idempotency keys and a single SoR integration prototype. Requires ADR + command surface.

**Grounding:** [`finance/reconciliation/matcher.rs`](src-tauri/src/finance/reconciliation/matcher.rs) (helper only).

---

### ADR-0012: Platform adapters are untrusted OS drivers behind policy gates

**Status:** Accepted

**Decision:** `platform/macos.rs`, `platform/windows.rs`, and `platform/headless.rs` perform OS I/O only. They do not evaluate policy, issue approvals, or write audit entries. Authorization decisions occur before platform dispatch.

**Context:** Renaming to "Ghost Agents" or "execution data plane products" adds no invariant. Adapters must remain replaceable and bounded (no network/shell in stable path).

**Alternatives:**
- Versioned agent products with separate release train — deferred.
- In-platform policy — rejected.

**Tradeoffs:** Monolithic repo vs. clear TCB audit surface.

**Consequences:** Platform changes require resolution benchmark and replay invariant tests. No separate agent protocol.

**Future Reconsideration:** Extract agents only if cross-machine remote execute is required — conflicts with local authority ADR-0001 unless control plane stays local.

**Grounding:** [`platform/`](src-tauri/src/platform/), [`engine.rs`](src-tauri/src/engine.rs), [`macos-automation-architecture.md`](macos-automation-architecture.md) (Swift adapters stay untrusted OS drivers; Rust keeps policy/approval).

---

### ADR-0013: MCP is an ingress adapter; not internal architecture

**Status:** Accepted

**Decision:** MCP stdio server is one **client** of Ghost's Organizer and diagnostics surface. It is not Ghost's spinal architecture. HTTP/TLS/relay remain experimental. `ghost.get_run` returns execution summary metadata — not cryptographic outcome verification.

**Context:** [`next-generation-architecture.md`](next-generation-architecture.md) elevated MCP excessively. Built stdio path routes through same plan/approve/execute as UI.

**Alternatives:**
- Ghost Execution Protocol as internal spine — deleted.
- MCP executes without tokens — rejected.

**Tradeoffs:** Interoperability vs. maintaining two ingress paths (UI + MCP) to same backend.

**Consequences:** New external integrations prefer MCP tools over bespoke provider adapters. Doc drift in `core-boundaries.md` corrected to "run summary metadata."

**Future Reconsideration:** Stable public envelope only if ≥3 independent clients need more than MCP tool schemas.

**Grounding:** [`mcp/server.rs`](src-tauri/src/mcp/server.rs), [`mcp/handlers.rs`](src-tauri/src/mcp/handlers.rs), [`tests/mcp_integration.rs`](src-tauri/tests/mcp_integration.rs).

---

### ADR-0014: Organizer is the first transaction compiler; Routines must converge

**Status:** Accepted

**Decision:** Ghost Organizer is the reference implementation of the trust pipeline. Routines/replay are a second **client** that must converge to the same policy, approval, audit, and undo standards — not a peer product with weaker guarantees.

**Context:** [`PROJECT_STATE.md`](PROJECT_STATE.md) §10 identifies Routines gap as biggest demo-to-product distance. Architecture review #14 warns of two products without one authority.

**Alternatives:**
- Perpetual split: Organizer = files, Routines = macros — rejected; fractures trust brand.
- Deprecate Routines — rejected; strategic second layer.

**Tradeoffs:** Significant engineering to route compression → guard → policy → undo vs. unified category story.

**Consequences:** Phase 2 milestone in debt register is blocking for "category-defining" claim beyond Organizer wedge.

**Future Reconsideration:** N/A — convergence is required, not optional.

**Grounding:** [`organizer/`](src-tauri/src/organizer/), [`core/compression/`](src-tauri/src/core/compression/), [`core/guard.rs`](src-tauri/src/core/guard.rs).

---

### ADR-0015: `Capability` is the policy action vocabulary until unified IR is proven

**Status:** Accepted

**Decision:** `policy::Capability` is the canonical mutation vocabulary for policy and Organizer plans. `CompressedStep` is the semantic replay vocabulary. A unified **Action IR** is a prototype goal, not a shipped subsystem. Do not add a parallel IR crate without proof that all paths map to it.

**Context:** Three coexisting representations today: `Capability`, `PlanAction`/`OrganizerPlan`, `CompressedStep`. No `ActionIR` type exists.

**Alternatives:**
- Big-bang Action IR rewrite — rejected.
- Proof-carrying obligations on every action — deferred until IR unification prototype passes.

**Tradeoffs:** Mapping maintenance vs. premature abstraction.

**Consequences:** New Organizer actions extend `Capability` first. Replay bridge in `policy/routines.rs` stays authoritative for routine policy.

**Future Reconsideration:** Phase 6 prototype in debt register: exhaustive map + property tests. ADR amendment on success.

**Grounding:** [`policy/capability.rs`](src-tauri/src/policy/capability.rs), [`organizer/planner.rs`](src-tauri/src/organizer/planner.rs), [`core/compression/types.rs`](src-tauri/src/core/compression/types.rs).

---

### ADR-0016: Triggers and schedulers never call execution

**Status:** Accepted

**Decision:** Event-sourced schedulers, folder watchers, and timer triggers may enqueue **intent candidates** for user review. They **never** call the execution plane directly. No scheduler subsystem in architecture.

**Context:** [`architecture-review-2031.md`](architecture-review-2031.md) and Hazel-class competitors auto-execute. Ghost invariant: user approves mutations.

**Alternatives:**
- Trusted auto-run on low-risk class A — rejected near-term; approval fatigue solved via `TrustLevel::Automate` on narrow rules, not watchers.

**Tradeoffs:** Less "hands-off" marketing vs. trust alignment.

**Consequences:** `enterprise/scheduling/` stub deleted from active architecture. No cron IPC.

**Future Reconsideration:** Only with formal proof that Class A reads cannot be confused with mutations and user enables per-zone auto-scan (plan-only).

---

### ADR-0017: Plugins deleted; intent arrives via MCP, UI, and CLI only

**Status:** Accepted

**Decision:** No plugin system, WASM marketplace, or third-party execution modules. Intent ingress: desktop UI, MCP stdio, Tauri CLI commands, and (experimental) HTTP webhook intents that **record only** — never auto-execute.

**Context:** Plugins duplicate MCP composition surface and expand TCB without customers.

**Alternatives:**
- WASM intent-only plugins — deleted.
- Native plugin DLLs — rejected.

**Tradeoffs:** Ecosystem growth slower vs. security review surface bounded.

**Consequences:** Remove plugin references from active roadmap. [`core-boundaries.md`](core-boundaries.md) experimental plugin SDK stays "not implemented."

**Future Reconsideration:** Unlikely. If composition demand exceeds MCP, revisit MCP tool registry design first.

---

### ADR-0018: Enterprise/fraud/compliance modules deleted from active architecture

**Status:** Accepted

**Decision:** Module trees `enterprise/` (except patterns reused internally), `checks/`, `fraud/`, `compliance/`, `data_protection/`, and finance stubs beyond read-only matcher are **removed from active architecture**. They remain in tree as dead scaffolding until follow-up PR removes compilation from default build. No Tauri commands without playbook + ADR.

**Context:** [`enterprise-financial-operations.md`](enterprise-financial-operations.md) states commandless scaffolding. `lib.rs` exports imply product readiness falsely.

**Alternatives:**
- Ship financial vertical — rejected; no trust pipeline extension proven.
- Keep as "future option" in architecture — rejected; creates debt.

**Tradeoffs:** Smaller Rust module tree vs. lost placeholder types for demos.

**Consequences:** Follow-up PR: `#[cfg(feature = "enterprise_scaffold")]` or deletion. Debt register tracks.

**Future Reconsideration:** One vertical with signed customer, playbook, and full trust pipeline for that workflow — new ADR per vertical.

**Grounding:** [`enterprise-financial-operations.md`](enterprise-financial-operations.md), [`lib.rs`](src-tauri/src/lib.rs).

---

### ADR-0019: Evidence export redacts PII; stored audit chain is unredacted

**Status:** Accepted

**Decision:** `audit/pii.rs` masks SSN/card/email/phone patterns on **exported** audit text only. Stored audit log and hash chain used for seal and undo remain unredacted so redaction cannot desync verification.

**Context:** Organizer export and Power BI preview use `pii::mask`. Separate `intelligence/redaction` for model-bound content.

**Alternatives:**
- Redact stored audit — rejected; breaks hash chain.
- No export redaction — rejected; enterprise export safety.

**Tradeoffs:** Two redaction pipelines to maintain vs. correct seal integrity.

**Consequences:** Export commands document redaction scope. Receipt v0 uses stored chain for verify.

**Future Reconsideration:** Field-level encryption at rest for sensitive audit fields — requires ADR on key management.

**Grounding:** [`audit/pii.rs`](src-tauri/src/audit/pii.rs), [`organizer_export_audit`](src-tauri/src/commands/organizer.rs).

---

### ADR-0020: Experimental features stay behind `--features experimental`

**Status:** Accepted

**Decision:** AI workflow generation, observer mode, cloud sync, analytics, Power BI push, Fabric/GCS integrations, MCP HTTP/relay, and intelligence providers compile and register only with `--features experimental`. Default build exposes trusted core only.

**Context:** `lib.rs` cfg gates; `is_experimental_enabled` IPC; `ipc_contract.rs` tests frontend gates.

**Alternatives:**
- Ship experimental in default — rejected.
- Separate binary — rejected; feature flag sufficient.

**Tradeoffs:** CI does not cover experimental leg by default; local validation required when touching gated code.

**Consequences:** PR template experimental checkbox. No experimental UI without gate check.

**Future Reconsideration:** Per-feature promotion to stable requires ADR + tests + threat model update.

**Grounding:** [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs), [`tests/ipc_contract.rs`](src-tauri/tests/ipc_contract.rs).

---

### ADR-0021: Business correctness and regulatory compliance are outside Ghost

**Status:** Accepted

**Decision:** Ghost does not claim economic correctness, fraud determination, regulatory compliance, settlement finality, or human judgment quality. It records what it executed and what evidence it collected; humans and systems-of-record own business truth.

**Context:** [`financial-transaction-authority.md`](financial-transaction-authority.md) §1 G6 Outside. Marketing overstatement identified in product audits.

**Alternatives:**
- "Compliance platform" positioning — rejected.
- Automated KYC/AML product — rejected (`compliance/` stubs deleted).

**Tradeoffs:** Narrower enterprise sales vs. survivable honesty under audit.

**Consequences:** Claims boundary in README and site must match. Filing preview is path suggestion only.

**Future Reconsideration:** Never for compliance determination. May aid evidence collection with human sign-off.

---

### ADR-0022: Architecture freeze until Organizer and Routines share one execution authority

**Status:** Accepted

**Decision:** **Freeze** new architectural subsystems until Phase 2 milestone (authority convergence) and Phase 3 milestone (resource drift invalidation) close with passing proofs. New work is implementation, tests, and ADR amendments backed by prototypes — not vision documents.

**Context:** Debt register verdict: conditional YES for category-defining product if engineering replaces design. Fourteen subsystems deleted from active architecture.

**Alternatives:**
- Continue architecture passes — rejected; reduces ship probability.
- Rewrite from scratch — rejected; Organizer seed is sufficient.

**Tradeoffs:** Slower narrative expansion vs. higher execution focus.

**Consequences:** No new `architecture-review-*` docs. Amend this ADR file and debt register only.

**Future Reconsideration:** Unfreeze requires: (1) Phase 2 + 3 proofs green, (2) signed decision from principal engineer, (3) written justification in ADR-0022 amendment.

**Grounding:** [`architecture-debt-register.md`](architecture-debt-register.md) §10.

---

## ADR index

| ID | Title | Status |
|---|---|---|
| ADR-0001 | Local transaction authority (restricted scope) | Accepted |
| ADR-0002 | Plan + policy + approval for all mutations | Accepted |
| ADR-0003 | AI intent only | Accepted |
| ADR-0004 | Pure Rust policy (not Cedar) | Accepted |
| ADR-0005 | Verification in executor | Accepted |
| ADR-0006 | Approval + tokens (not leases) | Accepted |
| ADR-0007 | FS binding strong; UI best-effort | Accepted |
| ADR-0008 | Server-side re-plan | Accepted |
| ADR-0009 | Hash chain = receipt v0 | Accepted |
| ADR-0010 | Undo-before-mutate | Accepted |
| ADR-0011 | No reconciliation engine | Accepted |
| ADR-0012 | Untrusted platform adapters | Accepted |
| ADR-0013 | MCP as ingress only | Accepted |
| ADR-0014 | Organizer first; Routines converge | Accepted |
| ADR-0015 | Capability vocabulary until IR proven | Accepted |
| ADR-0016 | No scheduler execution | Accepted |
| ADR-0017 | No plugins | Accepted |
| ADR-0018 | Enterprise modules deleted from architecture | Accepted |
| ADR-0019 | Export PII redaction | Accepted |
| ADR-0020 | Experimental feature gate | Accepted |
| ADR-0021 | Business correctness outside Ghost | Accepted |
| ADR-0022 | Architecture freeze | Accepted |
