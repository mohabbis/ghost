# Ghost Permanent Architecture Invariants

**Status:** Accepted (2026-07-13). Stable for the next decade unless amended via [`architecture-decision-records.md`](architecture-decision-records.md).  
**Audience:** Every engineer, agent, and reviewer touching Ghost code or docs.  
**Related:** [`architecture-debt-register.md`](architecture-debt-register.md), [`trust-pipeline.md`](trust-pipeline.md), `AGENTS.md`.

These twelve laws describe what Ghost **is** and **refuses to become**. They are derived from implemented behavior (Organizer, policy, audit, MCP) and from deleted architectural fantasy (leases, reconciliation engine, plugins, bank-grade GCM as product).

If a proposed change violates a law, the change is rejected unless an ADR amends the law with prototype proof.

---

## The twelve laws

### 1. AI may propose intent; it never executes actions.

Models and external AI clients suggest classifications, labels, plans, and explanations. They do not approve, dispatch OS operations, hold standing authority, or bypass policy. Deterministic Ghost code executes only after human approval.

*Grounding:* ADR-0003, `intelligence/`, `mcp/approval.rs`.

---

### 2. Every mutating operation requires a deterministic plan, policy check, and explicit approval.

No shortcut path mutates user state. The plan is produced by Ghost backend logic. Each action is evaluated through `policy::evaluate`. Approval is recorded in provenance (`Automated` only when policy explicitly allows; otherwise `UserApproved`).

*Grounding:* ADR-0002, [`trust-pipeline.md`](trust-pipeline.md).

---

### 3. The backend re-validates at execution time; client-supplied plans are not trusted.

The frontend and MCP clients may preview plans. Mutating commands re-plan or re-validate server-side before any filesystem or OS effect. A stale, tampered, or drifted plan cannot reach execution silently.

*Grounding:* ADR-0008, `organizer_execute`.

---

### 4. Policy is pure, deny-by-default, and evaluated before and during execution.

Policy evaluation has no IO and no side effects. Default outcome is deny. Organizer planner and executor both call the policy engine. Trust levels (`Automate`, `AskFirst`, `Never`) modulate prompting, not the existence of policy.

*Grounding:* ADR-0004, `policy/engine.rs`.

---

### 5. Reversible mutations write undo data before the mutation occurs.

For operations Ghost classifies as reversible (filesystem move/rename/mkdir in Organizer), undo journal entries are persisted before the mutation. Crash mid-run leaves a recoverable WAL record. Undo never performs recursive delete.

*Grounding:* ADR-0010, `organizer/executor.rs`, `audit/undo_journal.rs`.

---

### 6. Ghost never silently overwrites or deletes user files.

Executor refuses existing targets. No silent delete in Organizer path. Conflicts surface in plan review. User must approve explicit mutations.

*Grounding:* `organizer/executor.rs`, [`organizer-executor.md`](organizer-executor.md).

---

### 7. Audit records are append-only and tamper-evident for completed runs.

Every executed action produces an audit event. Completed Organizer runs seal into a hash chain verifiable offline (`organizer_verify_audit_chain`). Export may redact PII; stored chain remains unredacted for seal integrity.

*Grounding:* ADR-0009, ADR-0019, `storage/executions.rs`.

---

### 8. Platform adapters perform OS I/O; they do not decide authorization.

macOS, Windows, and headless backends capture and replay input. They do not evaluate policy, mint approvals, or interpret business rules. Authorization completes before platform dispatch.

*Grounding:* ADR-0012, `platform/`.

---

### 9. External systems and UI automation are observed, not trusted as proof of business effect.

Filesystem hashes and path identity can support strong local verification. UI automation, browsers, email, and SaaS APIs produce observed signals at best. Ghost records outcomes honestly; it does not label screenshot-confirmed clicks as settlement, compliance, or economic truth.

*Grounding:* ADR-0007, ADR-0021, [`target-resolution.md`](target-resolution.md).

---

### 10. Recovery from partial failure is explicit — never silent retry of ambiguous effects.

Interrupted runs surface unfinished state (`organizer_check_unfinished_run`). User chooses undo or dismiss. Ambiguous external effects are not blindly retried. `unknown` and `skipped` are valid terminal outcomes.

*Grounding:* ADR-0011, [`organizer-executor.md`](organizer-executor.md) crash recovery.

---

### 11. Business correctness, compliance, and settlement finality belong outside Ghost.

Ghost executes and evidences bounded local operations. Fraud scores, KYC outcomes, journal correctness, payment settlement, and regulatory compliance are human or system-of-record responsibilities. Ghost may help collect evidence; it does not certify business truth.

*Grounding:* ADR-0021, [`financial-transaction-authority.md`](financial-transaction-authority.md) §1 (claims boundary).

---

### 12. Ghost refuses or skips execution when certainty drops below defined guarantees.

If policy denies, resource identity diverges, target state conflicts, vault is locked, or approval is absent/invalid, Ghost skips or aborts the action and records why. It does not degrade to silent success or force commit labels beyond collected evidence.

*Grounding:* ADR-0005, ADR-0007, `policy/decision.rs`, `file_identity.rs`.

---

## Non-goals (enforceable)

Ghost must **never**, in product code or default UI:

1. Grant execution authority to AI, plugins, schedulers, or remote clients.
2. Execute shell commands, model-generated scripts, or arbitrary code from provider output.
3. Collapse permission, capability, approval, and token into a single undifferentiated "allow."
4. Execute against stale resource state after approval without re-validation.
5. Pretend irreversible operations (send, submit, publish, pay) are undoable.
6. Let triggers, watchers, or timers call the execution plane directly.
7. Treat MCP or any external protocol as the internal source of truth for trust logic.
8. Run ambient observation (camera, microphone, always-on screen, silent email/browser monitoring).
9. Become a chatbot, visual workflow canvas product, or generic autonomous agent.
10. Ship enterprise financial mutations (post journal, release payment, approve invoice) without a vertical-specific ADR and trust pipeline extension.
11. Claim ACID, exactly-once business effects, or regulatory compliance without evidence class and adapter tier to support the claim.
12. Add cloud-first storage for workflow or organizer data as default behavior.

Experimental features behind `--features experimental` must remain labeled and gated (ADR-0020).

---

## What was removed from architecture (not from laws)

The following concepts are **not** part of Ghost's engineering architecture until an ADR and prototype resurrect them:

- Capability leases and M-of-N SoD runtime
- Reconciliation Engine product subsystem
- Evidence tiers E0–E5 as product surface
- Ghost Kernel / Resource Graph
- Platform Agents (renaming `platform/`)
- Event-sourced scheduler with execution
- Plugin system / WASM marketplace
- Ghost Execution Protocol as internal spine
- Cedar / unconstrained Rego policy language
- Assumption objects and multi-axis uncertainty UI
- Enterprise `checks/`, `fraud/`, `compliance/`, `data_protection/` as active product modules

See deletion register in [`architecture-debt-register.md`](architecture-debt-register.md) §9.

---

## Amendment process

1. Prototype the change with falsifiable tests (debt register §7).
2. Write or amend an ADR with Future Reconsideration Criteria satisfied.
3. Update this file only if a **law** changes — not for every implementation detail.
4. Do not add a fourth vision document; amend ADRs.

**Current freeze:** ADR-0022 — architecture frozen until Organizer and Routines share one execution authority and resource drift invalidation is proven.

---

## Reading order

1. This file — laws and non-goals  
2. [`architecture-decision-records.md`](architecture-decision-records.md) — decision history  
3. [`architecture-debt-register.md`](architecture-debt-register.md) — debt, proofs, milestones  
4. [`PROJECT_STATE.md`](PROJECT_STATE.md) — as-built snapshot
