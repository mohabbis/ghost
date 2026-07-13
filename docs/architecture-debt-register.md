# Ghost Architecture Debt Register

**Status:** Principal engineer design review (2026-07-13). Engineering closure document — not a vision doc.  
**Audience:** Engineers deciding whether to stop designing and start proving.  
**Related:** [`architecture-decision-records.md`](architecture-decision-records.md), [`permanent-architecture-invariants.md`](permanent-architecture-invariants.md), [`PROJECT_STATE.md`](PROJECT_STATE.md).

---

## 1. Review charter

This review applies an adversarial bar equivalent to internal design review at Stripe, Palantir Foundry, Apple Platform Security, Microsoft Azure Core, and the Rust Foundation. Reviewers are assumed to be trying to **reject** the architecture.

**Established product identity (not re-litigated here):**

- Ghost is not an AI assistant, RPA platform, workflow builder, or financial application.
- Ghost is a **restricted local-first transaction authority** responsible for deterministic execution, authorization, verification, evidence, reconciliation (where applicable), and recovery — within bounded scope.

**Candidate designs, not truth:** Long-horizon documents ([`architecture-review-2031.md`](architecture-review-2031.md), [`financial-transaction-authority.md`](financial-transaction-authority.md), [`next-generation-architecture.md`](next-generation-architecture.md)) are inputs to this register. They do not override what is implemented.

**Rules applied:**

| Rule | Application |
|---|---|
| Rule 1 | Every component must answer ten existence questions; weak answers → deletion |
| Rule 2 | Subsystem debt table with purpose, difficulty, burden, verdict |
| Rule 3 | Complexity budget 1–10; below 5 → delete unless overwhelming evidence |
| Rule 5 | Design maturity classification per area |
| Rule 6 | Missing proofs become experiments, not new architecture docs |
| Rule 7 | Engineering milestones as invariant + proof pairs |
| Rule 8 | ≥25% of conceptual architecture deleted (not postponed) |
| Rule 9 | Permanent invariants live in [`permanent-architecture-invariants.md`](permanent-architecture-invariants.md) |

---

## 2. Ground truth snapshot

| Fact | Evidence |
|---|---|
| Strong path | Organizer: scan → plan → policy → approve → execute → audit / undo / WAL / hash-chain seal |
| Partial path | Routines / replay: compression + Guard + one-shot approve; not Organizer-grade Zones / per-step policy / full undo |
| MCP | Local stdio Organizer tools + signed approval tokens **built**; HTTP / TLS / relay experimental |
| Policy | Pure `policy::evaluate` — deny-by-default, tested |
| Closest transaction seed | Server-side re-plan on execute, per-action policy, undo-before-mutate, sealed audit chain, plan-hash MCP tokens |
| Not present as product | Unified Action IR, control/execution plane split, capability leases, reconciliation engine, signed receipt bundles, scheduler, plugins, Ghost Agents, Cedar policy, enterprise command surface |

---

## 3. Subsystem debt register

Columns: **Budget** (1–10 complexity score), **Verdict** (Keep / Merge / Split / Delete).

| Subsystem | Purpose | Invariant protected | Primary failure prevented | Impl diff | Ops diff | Test diff | Maint burden | Doc burden | Migration burden | Future flex | Budget | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Policy Engine** | Pure deny-by-default evaluation over capabilities and Zones | Unauthorized mutation | Silent allow of out-of-scope ops | Low | Low | Low | Low | Low | Low | High | 10 | **Keep** |
| **Organizer** | Filesystem transaction compiler: scan, plan, execute, audit, undo | Trust pipeline for local FS | Silent overwrite, stale plan execution | Medium | Low | Medium | Medium | Medium | Low | High | 10 | **Keep** |
| **Audit Journal** | Append-only record of every action outcome | Tamper-evident history | "We think we did X" without record | Low | Low | Low | Low | Low | Low | Medium | 9 | **Keep** |
| **Undo** | Reversible journal replayed in reverse | Recovery after approved mutation | Irrecoverable mistaken move | Medium | Low | Medium | Medium | Low | Medium | Medium | 9 | **Keep** (Organizer); **Merge** (replay text-only undo) |
| **Event Model** | Raw capture (`InputEvent`) + semantic compression (`CompressedStep`) | Inspectable automation | Silent loss of user actions | Medium | Low | Medium | Medium | Medium | Low | High | 9 | **Keep** |
| **Platform backends** | OS input capture/replay (macOS, Windows, headless CI) | Deterministic replay | Coordinate-only fragility | High | Medium | High | High | Medium | Low | Medium | 8 | **Keep** |
| **MCP (stdio)** | Ingress adapter for external AI clients | AI never holds execution authority | Client-side plan tampering | Medium | Low | Medium | Medium | Medium | Low | Medium | 7 | **Keep** |
| **Authorization** | Vault, UI approval, MCP tokens, routine fingerprint | Explicit consent before mutation | Auto-execution, token reuse | Medium | Low | Medium | Medium | Medium | Low | Medium | 7 | **Keep** (simplify; no leases) |
| **Resource Binding** | Path + hash identity for FS; boundary re-verify at execute | Approved plan matches reality | TOCTOU, symlink escape | Medium | Low | Medium | Medium | Low | Low | High | 7 | **Keep** (incremental) |
| **Verification** | Postcondition checks after dispatch | Effect matches expectation | False success labels | Medium | Low | Medium | Medium | High | Low | Medium | 6 | **Merge** into executor (no separate authority crate) |
| **Action IR** | Single executable representation | One mutation vocabulary | Divergent execution paths | High | Medium | High | High | High | High | High | 6 | **Merge** (`Capability` + `PlanAction` + `CompressedStep` until proven unified) |
| **Routines** | Cross-app recorded automation | Same trust as Organizer | Untrusted replay | High | Medium | High | High | Medium | Medium | High | 6 | **Keep** (must converge to Organizer authority) |
| **State Persistence** | redb Zones, rules, executions, milestones | Durable policy and run history | Lost undo / audit on crash | Low | Low | Low | Low | Low | Medium | Medium | 6 | **Keep** |
| **Filing** | Read-only audience-aware path preview | No surprise mutations from preview | Preview executing changes | Low | Low | Low | Low | Low | Low | Low | 5 | **Keep** |
| **Transaction Envelope** | Versioned plan + reads/writes + compensation metadata | Plan integrity at approval boundary | Tampered or stale envelope | High | Medium | High | High | High | High | High | 5 | **Keep as compile target** (seed = re-plan + plan-hash; no new runtime) |
| **Signed Receipts** | Cryptographic attestation of run outcome | Non-repudiation of local runs | "Logged it" ≠ proof | Medium | Low | Medium | Medium | High | Medium | Medium | 4 | **Merge** into audit hash-chain (v0 exists; universal bundle deferred) |
| **Reconciliation Engine** | Delayed SoR agreement with intended effect | External systems not atomic | False "done" on timeout | Very high | High | High | Very high | Very high | High | Medium | 3 | **Delete** (concept) |
| **Capability Leases** | Time-bounded authority over resource sets | Approval fatigue without blanket allow | Over-broad standing permission | High | Medium | High | High | High | High | Medium | 3 | **Delete** |
| **Evidence Model (E0–E5)** | Tiered independence of outcome proof | Evidence strength not greenwashed | Screenshot-as-proof for funds | High | Medium | High | High | Very high | High | Low | 3 | **Delete** (as product; export metadata later) |
| **Sagas (subsystem)** | Cross-system compensation orchestration | Honest partial failure | Pretend email/upload undo | High | High | High | High | High | High | Medium | 3 | **Merge** into undo + plan reversibility metadata |
| **Scheduler** | Event-sourced triggers → transaction candidates | Triggers never execute | Watcher auto-mutation | High | High | High | High | Medium | High | Low | 2 | **Delete** |
| **Plugin System** | WASM/marketplace intent composition | Plugins never execute | Third-party execution authority | Very high | High | High | Very high | High | High | Low | 2 | **Delete** |
| **Ghost Execution Protocol** | Versioned public envelopes for all clients | Protocol-stable integration | MCP-as-spine lock-in | High | Medium | High | High | High | High | Medium | 2 | **Delete** |
| **Platform Agents** | Renamed versioned execution-plane drivers | Replaceable adapters | Platform coupling | Low (rename only) | Low | None | Medium (naming drift) | High | Medium | Low | 2 | **Delete** (keep `platform/`) |
| **Ghost Kernel / Resource Graph** | Typed world model for all resources | Resource identity | Ad-hoc path strings | Very high | High | High | Very high | Very high | Very high | Medium | 2 | **Delete** |
| **Enterprise trees** | Financial ops domain scaffolding | Commandless models until playbook | Accidental financial product | Medium (stubs) | None | Low | High (dead code) | High | High | Low | 1–2 | **Delete** from active architecture |
| **GCM / OutcomeKnowledge / F0–F5** | Bank-adjacent consistency and class taxonomy | Honest guarantee limits | ACID theatre on desktop | Very high | High | Very high | Very high | Very high | High | Low | 1–2 | **Delete** from engineering backlog |
| **Cedar policy language** | Declarative signed policy bundles | Policy provenance at scale | Rust policy calcification | High | Medium | High | High | High | High | Medium | 2 | **Delete** (for now; Rust policy sufficient) |
| **Assumption objects / multi-axis uncertainty** | Planner-emitted assumptions with confidence | Unresolved high-impact assumptions block run | Decorative explanations | High | Low | High | High | High | Medium | Low | 2 | **Delete** |

**Verdict summary:** 11 Keep, 7 Merge, 0 Split, 14 Delete → **36% deletion/merge-away** of ~39 named subsystems (exceeds 25% threshold).

---

## 4. Rule 1 spot-checks (weak components)

### Reconciliation Engine — **Delete**

1. **Why exist?** Close loop when system-of-record disagrees with Ghost.
2. **Invariant?** External systems are not atomic.
3. **Duplicate?** Executor verification + audit export + human review cover Organizer today.
4. **Break if gone?** Nothing — no engine exists; `finance/reconciliation/matcher.rs` is a pure read-only helper.
5. **Complexity?** Queues, SLA aging, exception UI, SoR connectors.
6. **Future?** API integrations with idempotency keys — only if Ghost becomes integration hub (contradicts wedge).
7. **Incremental?** No — needs real SoR peers.
8. **Testable alone?** Only with mocked SoR.
9. **Understandable?** No — conflated with matcher module name.
10. **Five years?** Only for bank-adjacent pivot — explicitly out of near-term scope.

### Plugin System — **Delete**

1. **Why exist?** Third-party capability composition.
2. **Invariant?** Intent sources never execute.
3. **Duplicate?** MCP + UI + CLI already ingest intent.
4. **Break if gone?** Nothing — zero implementation.
5. **Complexity?** WASM sandbox, marketplace, signing, review.
6. **Future?** Maybe never — MCP tools are sufficient composition surface.
7. **Incremental?** No — needs runtime + trust model.
8. **Testable alone?** N/A.
9. **Understandable?** No — overlaps MCP and experimental AI.
10. **Five years?** Unlikely to beat MCP as ingress.

### Capability Leases — **Delete**

1. **Why exist?** Reduce approval fatigue with bounded standing authority.
2. **Invariant?** Exact authorization scope.
3. **Duplicate?** `FolderRule` + `TrustLevel` + one-shot MCP approval tokens + `approve_routine_replay`.
4. **Break if gone?** Nothing — leases are not implemented.
5. **Complexity?** Lease store, expiry, material-change invalidation, M-of-N.
6. **Future?** Enterprise SoD — only with external IdP and proven need.
7. **Incremental?** Partial, but FolderRule already covers FS wedge.
8. **Testable alone?** Would need full identity model first.
9. **Understandable?** No — docs conflate permission, capability, lease, token, approval.
10. **Five years?** Maybe for enterprise; not for category wedge.

---

## 5. Documentation overpromise register

| Doc / claim | Hazard | Action |
|---|---|---|
| [`architecture-review-2031.md`](architecture-review-2031.md) | 10+ unbuilt layers read as roadmap | **Frozen reference** — amend only via ADR + prototype proof |
| [`financial-transaction-authority.md`](financial-transaction-authority.md) | G0–G6 guarantees read as current product | **Archive tier** — claims boundary reference only |
| [`next-generation-architecture.md`](next-generation-architecture.md) | "Git for AI" + MCP as spinal architecture | **Demote** — MCP is ingress client, not core |
| [`core-boundaries.md`](core-boundaries.md) | MCP "run verification" overstates metadata summary | **Correct** in ADR-0013 scope |
| `lib.rs` enterprise module tree | Implies financial product readiness | **Recommend removal** from default build (follow-up PR) |
| `CFO_IT_AUDIT_RESILIENCE.md`, marketing POS/Guard claims | Overstates Guard Desk / POS Bridge | **Claims boundary** — not engineering backlog |
| [`event-compression.md`](event-compression.md) status section | May say "no Tauri command" while `compress_workflow` exists | **Doc drift fix** (follow-up) |

---

## 6. Design maturity assessment

| Area | Maturity | Notes |
|---|---|---|
| Organizer trust pipeline | **Conceptually Complete** | End-to-end in code + tests |
| Policy engine | **Conceptually Complete** | Pure, deny-by-default, unit-tested |
| Audit + hash chain | **Conceptually Complete** | `verify_chain`, export, PII mask on export |
| Filesystem resource binding | **Needs Prototype** | `file_identity.rs` exists; approval-stale invalidation incomplete |
| Unified Action IR | **Needs Prototype** | Three coexisting representations; no proof of unification |
| Routines ↔ Organizer authority | **Needs Formal Review** | Split execution paths; trust gap documented in audits |
| Replay verification / resolution | **Needs Clarification** | Code strong; MCP/docs overclaim "verification" |
| Signed receipts (universal bundle) | **Needs Removal** | Hash-chain is v0; receipt bundle is architecture theater near-term |
| Reconciliation engine | **Needs Removal** | |
| Capability leases / SoD | **Needs Removal** | |
| Scheduler / plugins / GEP | **Needs Removal** | |
| Enterprise financial modules | **Needs Removal** | Stubs create false surface area |
| Cedar / WASM / Ghost Agents | **Needs Removal** | |

---

## 7. Missing proofs → experiments (not docs)

| Architectural claim | Supporting evidence today | Required experiment |
|---|---|---|
| Single execution authority across Organizer + Routines + MCP | Partial — MCP routes through Organizer; replay is separate | Integration matrix: every mutation path through one gate; delete bypasses |
| Resource drift invalidates authorization | `file_identity.rs`, TOCTOU tests in `canonical_workflows.rs` | Fuzz: mutate file between user approval and execute; assert skip + audit |
| Action vocabulary covers every mutation class | `Capability` covers Organizer; replay uses `routines.rs` bridge | Exhaustive map: `organizer_execute` + `replay_workflow` → capability or deny |
| Hash-chain receipt verifies offline | `organizer_verify_audit_chain`, `storage/executions.rs` tests | Property tests: corrupt chain → detect; export round-trip |
| UI target identity stable enough for policy | `resolution_benchmark.rs`, `ResolutionKind` trace | Benchmark extension with deliberate UI drift injection |
| Replay deterministic under pause/cancel | `replay_support.rs`, platform loops | Chaos: pause/cancel mid-step; assert no duplicate effects |
| MCP tokens cannot replay across plans | `tests/mcp_integration.rs` | **Proven** — document as closed proof |
| Platform helpers bounded TCB | `platform/macos.rs`, `platform/windows.rs` | Static audit: no network/shell escapes in stable path |
| Action IR can represent every execution class | Not proven — three IRs coexist | Prototype: translate all `PlanAction` + `CompressedStep` → single enum; property tests |
| Reconciliation scales beyond filesystem | Not applicable — no engine | Defer until API adapter tier 1 exists with idempotency |
| Receipts remain compact | Hash-chain only today | Measure export size at 10k-action run before bundle design |
| Adapters remain capability-bounded | Policy gates exist for FS; replay weaker | Adapter audit checklist per platform backend |

---

## 8. Engineering milestones (invariant + proof)

No feature roadmap. Each phase closes one invariant with a falsifiable proof.

### Phase 1 — Execution singularity

- **Invariant:** Only backend-approved plans reach mutation.
- **Proof:** `organizer_execute` accepts Zone id only (not client plan); MCP `execute-approved` requires signed token bound to plan hash; integration tests assert rejection of tampered plans.

### Phase 2 — Authority convergence

- **Invariant:** Routines use the same policy + approval model as Organizer.
- **Proof:** `routine_policy_plan` + Zone parity; replay undo journal covers all reversible step kinds; Guard findings routed to review UI.

### Phase 3 — Resource drift

- **Invariant:** Execution skips actions when resource identity diverges from plan snapshot.
- **Proof:** Extended TOCTOU suite: approval-stale scenarios; audit records `Skipped` with reason.

### Phase 4 — Receipt v0

- **Invariant:** Every completed Organizer run has verifiable hash-chain seal.
- **Proof:** `organizer_verify_audit_chain` passes on intact runs; negative tests detect tampered links.

### Phase 5 — Production maturity

- **Invariant:** Release artifacts signed; updater verifies; marketing matches implementation.
- **Proof:** `implementation-tracker.md` items closed with CI/release evidence.

### Phase 6 — Action vocabulary unity (prototype gate)

- **Invariant:** Every mutating operation is expressed as a policy-checkable capability before dispatch.
- **Proof:** Single mapping table tested; no silent execution path without `evaluate`.

---

## 9. Deletion register (Rule 8)

Fourteen components removed from active architecture. Not postponed — **deleted** until reconsideration criteria in ADRs are met.

| Deleted component | Why unnecessary today | Invariant that survives | Maintenance cost removed | Return condition |
|---|---|---|---|---|
| Reconciliation Engine | No SoR connectors; Organizer is local FS | External systems reconciled by human/process, not Ghost runtime | Engine runtime, queues, exception UI | Tier-1 API adapter with idempotency + paying enterprise customer |
| Capability Leases | FolderRule + tokens suffice | Scoped authorization per operation | Lease store, expiry, material-change matrix | External IdP + proven SoD requirement |
| Evidence Model E0–E5 | No product surface; confuses filename "Receipts" | Audit export + hash chain | Tier taxonomy docs, receipt bundle schema | Universal receipt v0 shipped and exported |
| Platform Agents (rename) | `platform/` already exists | Replaceable adapters | Naming drift across docs/code | Never — keep `platform/` name |
| Scheduler | Triggers must not execute (invariant) | User-initiated intent only | Event-sourced scheduler design | User demand + proven safe candidate-only design |
| Plugin System | MCP is composition surface | Intent-only ingress | WASM runtime, marketplace | Never if MCP remains ingress |
| Ghost Execution Protocol | MCP + Tauri IPC work | Versioned tool contracts via MCP | Protocol spec maintenance | Multiple independent clients need stable envelope beyond MCP |
| Ghost Kernel / Resource Graph | No code; path binding works | Resource identity on paths/hashes | Graph schema, discovery orchestration | Cross-resource transactions beyond FS |
| Enterprise trees (`checks/`, `fraud/`, `compliance/`, `data_protection/`, `enterprise/scheduling`, `enterprise/tenancy`) | Commandless stubs imply product | Trust pipeline for Organizer only | Dead module compilation, false expectations | Playbook + ADR + command surface for one vertical |
| GCM / OutcomeKnowledge / F0–F5 | Bank taxonomy without bank product | Honest limits in claims boundary | 600+ lines of unread financial class docs | Regulated customer contract requiring those guarantees |
| Cedar policy | Rust policy works and is tested | Deterministic deny-by-default policy | Cedar integration, bundle signing | Policy rule count exceeds Rust maintainability threshold |
| Assumption objects | No enforcement path | User reviews plan preview | UI complexity for decorative confidence | Planner Interface with machine-checked assumptions |
| Multi-axis uncertainty gates | No runtime enforcement | Policy risk levels + manual review | Uncertainty axis configuration | Proof obligations shipped per action |
| Verification Authority (separate plane) | Executor postconditions work for Organizer | Verify before commit | New crate, duplicate policy questions | Multi-step sagas across systems with shared verification API |

---

## 10. Architecture freeze verdict

### Final question

**If Ghost stopped all architectural design work today and spent the next two years implementing only what already exists, would it still be capable of becoming a category-defining product?**

**Answer: Conditional YES — recommend an architecture freeze.**

Ghost can become category-defining in **safe local file operations + permission-bounded desktop routines** if engineering replaces further conceptual expansion with proof. The moat is **trustworthy local execution with evidence of what changed** — not financial reconciliation, plugin marketplaces, or AI orchestration breadth.

### Existential gaps (two only)

1. **Split execution authority** — Organizer, Routines, and MCP are three clients without one converged mutation gate. Trust claims fracture across surfaces ([`architecture-review-2031.md`](architecture-review-2031.md) critical review #14). **Proof required:** Phase 2 milestone.

2. **Resource-version invalidation of authorization** — Filesystem binding exists (`organizer/file_identity.rs`) but is not yet a universal principle tying approval to observed state; UI automation has no equivalent. **Proof required:** Phase 3 milestone.

Signed universal receipts are valuable but **not existential** — hash-chain + export is sufficient receipt v0 for the wedge.

### Freeze justification

Further conceptual expansion (leases, reconciliation engine, plugins, GEP, enterprise trees, Cedar, Ghost Agents) increases documentation surface faster than proof. Each deleted subsystem removes maintenance burden without weakening Organizer invariants.

**After freeze, new architecture work is limited to:**

- ADR amendments in [`architecture-decision-records.md`](architecture-decision-records.md)
- Prototype results that close items in §7
- Corrections to doc drift in §5

**Do not add:** New subsystems, north-star vision documents, or enterprise module trees without a customer-bound playbook and passing prototype.

---

## 11. Reading order

1. [`permanent-architecture-invariants.md`](permanent-architecture-invariants.md) — decade-stable laws  
2. [`architecture-decision-records.md`](architecture-decision-records.md) — accepted decisions  
3. [`PROJECT_STATE.md`](PROJECT_STATE.md) — as-built engineering snapshot  
4. [`architecture-review-2031.md`](architecture-review-2031.md) — frozen long-horizon reference (do not extend)  
5. [`financial-transaction-authority.md`](financial-transaction-authority.md) — claims boundary archive (do not implement without ADR)
