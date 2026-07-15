# Ghost as a Verifiable Transaction Authority for Regulated Financial Operations

**Status:** Fourth-pass architecture review (docs only). No implementation.  
**Audience:** Distributed-systems, financial-controls, operational-risk, formal-methods, security, and audit stakeholders.  
**Relationship:** Subordinate specialist review under [`architecture-review-2031.md`](architecture-review-2031.md). Does not replace the product north star; it pressure-tests whether that north star can survive consequential financial work spanning humans, AI planners, desktop apps, files, APIs, and systems without atomicity.  
**Non-goals:** Do not become core banking, RPA, GRC, AML detection product, model-governance dashboard, or a consulting toolkit of finance AI demos. Domain scaffolds in `enterprise/`, `finance/`, `fraud/`, `compliance/` remain commandless until a playbook + this review’s cut line says otherwise ([`enterprise-financial-operations.md`](enterprise-financial-operations.md)).

---

## 1. Executive conclusion

**Thesis under test:** Ghost governs the execution boundary between intent and consequential business state: it constructs a versioned transaction envelope, binds exact resources, evaluates policy and controls, obtains independently valid authorization, executes through constrained adapters, verifies observable effects, reconciles ambiguous outcomes, preserves tamper-evident evidence, and issues a signed attestation of what is proven and what remains uncertain.

**Attempted disproof:** Desktop UIs, browsers, email, spreadsheets, and most SaaS surfaces do not expose commit IDs, isolation, or truthful acknowledgements. Screenshot-confirmed clicks can lie. Local-only software cannot defend integrity against a malicious machine administrator. Humans rubber-stamp. AI planners invent resources. Cross-system work cannot be ACID. Therefore “verifiable transaction authority” collapses into theatre.

**Partial disproof succeeds — full disproof fails if Ghost stays honest.** Ghost *can* become a verifiable authority for a *restricted* class of operations: those for which adapters produce independent evidence, resource versions can be bound, outcomes admit indeterminate knowledge, reconciliation is first-class, and claims never inflate past evidence strength. Ghost *cannot* become a substitute for systems of record, settlement networks, or internal audit.

**Answer to the core question:**

| Kind | Position |
|---|---|
| Guarantees Ghost **can** provide | Intent provenance (channel, planner version, identity); plan integrity (hash-bound envelope); authorization integrity *within Ghost’s principal model*; execution integrity (only approved ops dispatched by Ghost); local exact effect verification for Tier-0 adapters; tamper-evident journals under non-admin adversaries; typed outcome knowledge; adapter-tier gating |
| Guarantees Ghost **can approximate** | Effect verification for API systems with idempotency keys and IDs; semantic-UI verification with confidence bounds; eventual reconciliation; multi-principal SoD; policy provenance; capability leases; temporal cutoffs; lineage of Ghost-touched artifacts |
| Guarantees Ghost **cannot** provide across third-party desktop/SaaS | True distributed atomicity; exactly-once external business effects without peer support; proof that the UI “really” matches hidden app state; economic correctness; regulatory “compliance”; settlement finality; human judgment quality; integrity under local admin compromise |
| Claims Ghost must **never** make | See §20 |
| Controls that reduce risk without fake ACID | VECTRA rejected; **Ghost Consistency Model (GCM)** + OutcomeKnowledge + Reconciliation Engine + evidence classes + adapter tiers + financial operation classes F0–F5 + fail-closed defaults |

**Smallest truthful foundation today (§22 cut line):** canonical transaction envelope, resource-version binding for local files, explicit OutcomeKnowledge (including indeterminate), evidence classes on receipts, adapter trust tiers enforced in policy, and a universal receipt schema that distinguishes Proven / Observed / Claimed / Unknown — without claiming bank-grade guarantees.

---

## 2. Guarantee model (G0–G6)

Definitions are operational. “Trusted/secure/compliant” are forbidden unless redefined here.

### Classification key

| Class | Meaning |
|---|---|
| **Strongly enforceable** | Ghost can prevent violation by refusing dispatch or refusing commit labeling |
| **Conditionally enforceable** | Enforceable under stated adapter / IdP / peer-system assumptions |
| **Observable but not provable** | Ghost can record signals; cannot establish truth of external state |
| **Outside Ghost’s authority** | Ghost must not claim; at most carries human certification as *asserted* |

### Guarantee definitions

| ID | Guarantee | Enforceability |
|---|---|---|
| **G0** Intent provenance | Who/what submitted intent; channel (UI/MCP/CLI/scheduler); device/session; planner/model + prompt/rule hashes; authentication state; whether intent mutated before planning | Conditionally enforceable (local principals strong; remote IdP conditional; model substitution often Observable) |
| **G1** Plan integrity | Exact serialized transaction reviewed; action-set hash; policy bundle hash; assumptions presented; bound resource versions; post-approval immutability (else reapproval) | Strongly enforceable inside Ghost |
| **G2** Authorization integrity | Approver principal(s); roles/delegations; SoD; scoped unexpired lease; requesters ≠ approvers where required; step-up auth | Conditionally enforceable (depends on identity source integrity) |
| **G3** Execution integrity | Only approved ops dispatched; known executor/adapter versions; ordered dispatch; Ghost did not intentionally emit unapproved ops | Strongly enforceable for what Ghost *sends*; Observable regarding whether OS delivered them |
| **G4** Effect verification | Exact local transition; semantic UI; API-confirmed mutation; external ID; later SoR reconciliation | Tier-dependent: Strong for local FS hash; Conditional for APIs; Observable for UI; Outside for settlement finality |
| **G5** Non-repudiation / evidence integrity | Evidence unaltered after capture; receipts bound to tx; tamper-evident order; signer + key provenance; retention/export lineage | Conditionally enforceable vs non-admin adversaries; Outside vs malicious local admin with raw disk and signing key |
| **G6** Business correctness | Economic validity, fraud correctness, settlement, well-informed approval, third-party mutations outside Ghost, screenshot fidelity to hidden state | **Outside Ghost’s authority** (human/SoR attestations may be *recorded*, not *proven*) |

### Matrix by resource class

| Resource class | Strongest realistic guarantee | Dominant failure |
|---|---|---|
| Local filesystem | G1–G4 Strong (hash/inode/volume); G5 Conditional | TOCTOU, symlink, concurrent human/cloud sync |
| Native desktop UI | G3 Strong; G4 Observable→Conditional (AX/UIA semantic) | Label spoofing, reuse of titles, unstable trees |
| Browser UI | G3 Strong; G4 mostly Observable (visual/OCR) | DOM drift, spoofed UI, no stable commit |
| API-backed SaaS | G3 Strong; G4 Conditional with idempotency + IDs | Timeout after accept; duplicates |
| Database Ghost writes | G4 Conditional (commit ID) if Ghost owns connection | Foreign writers |
| Spreadsheet | G4 Observable/Conditional (file hash ≠ cell semantics) | Formula side effects, linked workbooks |
| Email | G3 Strong; G4 AcknowledgedUnverified typical | SMTP accept ≠ delivery/read |
| Payment / funds movement | G3 Conditional; G4 outside without payment rail acknowledgements + reconciliation | Ambiguous timeout = possible double send |
| Reporting system | G4 Conditional with job IDs / refresh tokens | Stale cache presented as fresh |
| Human judgment | G2 records assent; G6 Outside | Rubber-stamp, fatigue, coercion |

---

## 3. Consistency model — reject “ACID-like”

[`architecture-review-2031.md`](architecture-review-2031.md) used “ACID-like.” **That phrase is rejected** unless each letter is redefined. Desktop-spanning work is not a database.

### Letter-by-letter rejection

| Property | Desktop reality | Ghost position |
|---|---|---|
| **Atomicity** | Impossible across Outlook + Excel + SaaS + FS | **Compensating atomicity** only: declare compensation plan; admit partial commit |
| **Consistency** | No shared schema of truth | **Policy + invariant + evidence consistency** inside Ghost’s envelope; SoR consistency via reconciliation |
| **Isolation** | Concurrent humans and sync clients | **Optimistic version checks + lease invalidation**; not serializable isolation |
| **Durability** | Local disk can be wiped | **Durable journal under ordinary crashes**; not durable vs admin wipe or disk death without archival |

### Ghost Consistency Model (GCM)

**VECTRA** (Version / Evidence / Capability / Tamper-evident / Reconciliation / Attested) is rejected as product branding. The properties remain; the name is unbranded:

| Property | Invariant |
|---|---|
| **Version-bound resources** | Approval binds resource versions; version drift ⇒ invalid lease |
| **Evidence-classified outcomes** | Every action carries required vs collected evidence class; receipts never greenwash |
| **Capability-scoped authorization** | Dispatch requires unexpired lease ∩ policy decision ∩ material-change check |
| **Tamper-evident execution** | Append-only journal + hash chain (+ optional external anchor later) |
| **Reconciliation-aware recovery** | Indeterminate/external effects enqueue reconciliation; not silent “success” |
| **Attested outcomes** | Signed receipt discloses Proven / Strongly evidenced / Observed / Inferred / Claimed / Unverified / Unknown |

Secondary properties Ghost must define explicitly:

| Term | Definition |
|---|---|
| Idempotency | Same `(tx_id, action_id, resource_version)` key ⇒ no duplicate *side effect attempt* when effect already Observed/Verified |
| Monotonic execution | Ghost never moves an action backward from Verified to NotDispatched without an explicit compensation transaction |
| Eventual verification | External effect may reach EffectVerified after AcknowledgedUnverified via reconciliation |
| Causal ordering | Journal order within a tx is total; cross-tx causality via event log edges |
| Exactly-once intent | Control plane dedupes intent ids |
| At-least-once dispatch | Possible under crash; must be safe under idempotency |
| Effectively-once business effect | Achievable only when peer supports idempotency keys / unique SoR keys |
| Partial commit | Allowed; must be labeled; compensation or escalate |
| Indeterminate outcome | First-class; forbids blind retry |

### Failure×guarantee behavior

| Scenario | Behavior |
|---|---|
| Normal execution | GCM full path; receipt |
| Process crash mid-tx | WAL/journal resume; states `unknown` until probed; no blind retry of Class F3+ |
| OS crash | Same; journal recoverability Conditional on durable flush |
| Network loss | API actions → DispatchedUnacknowledged; reconcile |
| Adapter timeout | Treat as indeterminate unless idempotent probe succeeds |
| Ambiguous external response | Indeterminate + reconciliation mandatory |
| Duplicated request | Idempotency key absorbs |
| Stale resource | Reject dispatch / invalidate lease |
| Partial cross-system success | Partial commit + saga compensate where declared |
| User interruption | Pause; no auto-continue past Class F2 without revalidation |
| Concurrent human mutation | Version clash; replan |
| Malicious/compromised planner | Planner outside execution plane; SoD + assumption gates; G6 still Outside |

### State machines (text)

**1) Local reversible (FS move, Class B / F1)**  
`Compile → BindVersions → Policy → Approve → Lease → Snapshot0 → Dispatch → EffectVerified(hash) → Commit → Receipt`  
On fail after move: compensate with reverse move if destination exclusive; else escalate.

**2) API with idempotency**  
`… → Dispatch(idem_key) → {Ack+ID → EffectVerified | Timeout → DispatchedUnacknowledged → ReconcileProbe → …}`  
Retry allowed **only** with same idempotency key until terminal.

**3) UI-driven ambiguous**  
`… → Dispatch(click) → AcknowledgedUnverified | EffectObserved(semantic/visual) → never auto-Commit as Proven`  
Unattended F3+ forbidden on Tier ≥3.

**4) Multi-system with compensation**  
`Step1 Verified → Step2 Verified → Step3 Fail → Compensate(reverse eligible steps) → Partial/Compensated receipt`  
Irreversible step already done ⇒ CompensationFailed / escalate.

**5) Delayed verification**  
`Dispatch → AcknowledgedUnverified → SettlementPending → Reconcile within deadline → EffectVerified | ExceptionQueue`  
Business “done” ≠ Ghost Commit label until reconciling authority certifies or policy allows provisional.

---

## 4. OutcomeKnowledge (first-class)

Candidate enum (adequate **if** coupled to retry/reconcile rules below):

```text
NotDispatched
DispatchRejected
DispatchedUnacknowledged
AcknowledgedUnverified
EffectObserved
EffectVerified
PartiallyObserved
Compensated
CompensationFailed
Diverged
Indeterminate
SettlementPending          # added: external accepted, finality pending
```

`SettlementPending` is required for payment-like and batch jobs; without it, teams collapse finality into “success.”

### Rules matrix (abridged)

| State | Retry? | Reconcile? | Human? | Duplicate risk | Incident? |
|---|---|---|---|---|---|
| NotDispatched | Yes (fresh lease) | No | No | Low | No |
| DispatchRejected | After fix | No | If policy | Low | Optional |
| DispatchedUnacknowledged | **Forbidden** blind; probe only | **Mandatory** | If past SLA | **High** | If aging |
| AcknowledgedUnverified | No effect retry | Mandatory until Verified or waived Class F0–F1 only | F3+ | Medium | Per policy |
| EffectObserved | No | Strengthen evidence | If evidence gap vs required | Low–Med | If required E≥3 missing |
| EffectVerified | No | Optional sampling | No | Low | No |
| PartiallyObserved | No | Yes | Yes | Med | Yes |
| Compensated | No (new tx if redo) | Yes | Review | Low | Optional |
| CompensationFailed | No | Yes | **Yes** | Med | **Yes** |
| Diverged | No | Yes | **Yes** | High | **Yes** |
| Indeterminate | Probe only | **Mandatory** | F2+ | **High** | Aging rules |
| SettlementPending | No | Until final/fail | Threshold-based | High if retry | Aging |

**Do not retry blindly** for: email send, form submit, journal post, report upload, transfer initiation, case-system create, batch job trigger.

---

## 5. Reconciliation Engine

Distinct from verification, compensation, audit, and policy.

| Concern | Verification | Reconciliation |
|---|---|---|
| Timing | Immediate pre/post step | Delayed, often async |
| Question | Did *this* action’s local/adapter obligations hold now? | Does *system of record* agree with intended *business* effect? |
| Authority | Executor-adjacent probes | Independent SoR, control totals, human certifier |
| Failure mode | Block next step / refuse Proven label | Exception queue, aging, escalate |

### Contract sketch

```text
reconciliation:
  system_of_record: finance_ledger
  expected:
    record_count: 148
    debit_total: "472391.19"
    credit_total: "472391.19"
    batch_hash: sha256:...
  match_strategy:
    keys: [account, amount, effective_date, source_reference]
  deadline: PT30M
  unresolved_action: escalate
```

### Examples

1. **Month-end package** — count/hash of evidence files vs package manifest; custodian acknowledgment.  
2. **File export/ingest** — SoR import job ID + row counts + hash totals.  
3. **Email/document delivery** — MTA accept ID ≠ read; recon may stop at provider accept + retention of MIME hash.  
4. **External submission** — portal confirmation number + later status poll.  
5. **Payment-like** — rail ID + settlement state; never “verified” on UI click alone.  
6. **Power BI / Fabric refresh** — dataset refresh ID + completion webhook/poll; receipt cites refresh status, not screenshot.

---

## 6. Control objectives vs instances

Ghost **executes and evidences** controls; it does **not** own the enterprise risk register (not a GRC app).

| Layer | Example |
|---|---|
| Objective | Externally submitted financial reports reviewed by authorized Finance manager |
| Definition | Requester ≠ approver; role `FinanceReportingApprover`; approve after final report hash; expire on content change |
| Instance | Q2 Liquidity Report, `tx_01822` |
| Execution | Approved by `p_091` at … |
| Evidence | Report hash, approval sig, role assertion, policy bundle hash |
| Exception | Break-glass with mandatory reason + secondary certifier |
| Certification | Periodic control owner attestation *outside* Ghost or via import |

Compatible with preventive / detective / corrective; manual / automated / IT-dependent manual; entity vs transaction-level; key vs compensating — as **tags on definitions**, not a full COSO product.

---

## 7. Separation of duties & multi-principal auth

### Principals (roles in a transaction lifecycle)

Requester, Planner, Preparer, Reviewer, Approver, Executor (runtime), Verifier, Reconciler, Administrator, Policy Author, Emergency Operator.

### Prohibited combinations (baseline)

| Forbidden | Why |
|---|---|
| Requester = sole final Approver | Classic SoD |
| Policy Author = sole Policy Deployment Approver | Self-dealing policy |
| Executor runtime ≠ human; human Executor role ≠ Reconciler certifier for same tx | Self-clearing |
| Model / planner provider cannot approve its proposal | G2 |
| Administrator cannot silently waive evidence requirements | Must leave break-glass trail + obligation |

Support: M-of-N, sequential/parallel approvals, RBAC, amount/class thresholds, break-glass, delegation, temporary authority, geo/device trust, step-up auth, **reapproval after material change**.

### Material change (invalidates approval)

Any of: resource hash/version; amount; recipient; account; control set; action count increase; reversibility weakened; verification strength degraded; planner assumptions changed; policy bundle version changed; temporal window crossed (cutoff/period close); adapter tier worsened.

---

## 8. Evidence strength hierarchy

| Class | Examples | Independence |
|---|---|---|
| **E0** Assertion only | Adapter “success” | Executor-dependent |
| **E1** Visual | Screenshot, OCR, on-screen toast | Weak; spoofable |
| **E2** Semantic application | AX/UIA tree, document metadata, native events | Medium |
| **E3** System | API response, DB commit ID, immutable external tx id | Strong *if* channel authenticated |
| **E4** Reconciled | Independent SoR match / control totals | Stronger |
| **E5** Independently attested | Signed external response, HSM/timestamp, separate verifier | Strongest Ghost can *carry* |

Hierarchy is correct **as an ordering of independence**, not as truth. E1 must never satisfy F4/F5 required evidence.

Receipt labels (never collapse): **Proven** (E≥3 local/API + obligations met) · **Strongly evidenced** · **Observed** · **Inferred** · **Claimed by adapter** · **Unverified** · **Unknown**.

Each action declares: required class, collected class, gaps, independence of executor, before/after commit timing, retention class, masking needs.

---

## 9. Policy decision provenance

“Allow” is insufficient. Required decision record:

```text
decision:
  result: allow_with_obligations   # deny | allow | allow_with_obligations
  policy_bundle_hash: sha256:...
  policy_version: 42
  evaluator_version: 3.1.0
  evaluated_at: ...
  principal_context_hash: ...
  resource_context_hash: ...
  transaction_hash: ...
  matched_rules: [FIN-REPORT-004, SOD-002]
  obligations: [manager_approval, retain_7y, reconcile_PT30M]
  warnings: [identity_confidence_low]
```

### Policy engine recommendation

| Option | Verdict |
|---|---|
| Hand-grown YAML | **Reject** |
| OPA/Rego unconstrained | **Reject** as default (too easy to create unsound sprawl) |
| Pure typed Rust forever | **Keep for Current Ghost**; saturates |
| **Cedar + signed policy bundles** | **Adopt for Financial-services-ready** — resource/principal/action/context fits leases |
| Hybrid: Rust embeds compiled Cedar bundle | **Recommended architecture** |

Must support: signed deployment, effective dating (no silent retroactive reinterpretation of old txs — historical eval uses *then* bundle hash), rollback, migration, simulation on historical envelopes, shadow eval, conflict resolution order, inheritance/overlays, logged exceptions, break-glass.

---

## 10. Adapter trust tiers

| Tier | Interface | Unattended F3+ | Funds F5 | Financial close F4 |
|---|---|---|---|---|
| **0** Deterministic local (FS hash, local DB Ghost owns) | Prefer | Conditional | No | Prep only |
| **1** Structured native/API (Graph, DB protocol, explicit API) | Conditional | Only with E≥3 + reconcile | Conditional with rail IDs | Prefer |
| **2** Semantic UI (AX/UIA) | Prefer attended | **No** | **No** | Prep/evidence assemble only |
| **3** Visual (OCR, template, coordinates) | **No** | **No** | **No** | **No** as proving path |
| **4** Unverifiable external | **No** | **No** | **No** | **No** |

Visual click ≠ API commit. Policy must encode that without reliance on folklore.

---

## 11. Financial operation classes (F0–F5)

Kernel remains domain-agnostic; classes are policy dimensions.

| Class | Examples | Min tier | Approval | Evidence | Reconcile | Model role |
|---|---|---|---|---|---|---|
| **F0** Informational | read, classify | any (prefer 0–2) | Low | E0–E2 | No | May propose |
| **F1** Internal reversible prep | organize working copies | 0–1 | Single | E3 local hashes | Optional | Suggest only |
| **F2** Internal record mutation | controlled workbook write | 0–1 | SoD optional | E2–E3 | Optional | Suggest only |
| **F3** External communication | email report, portal submit | ≤1 preferred; 2 attended | Dual often | E3 if possible else E1+human | Delivery SLA | No auto-send |
| **F4** Accounting-impacting | journal prep/post, cert | ≤1 | Dual + threshold | E3–E4 | **Yes** | No authority |
| **F5** Value transfer / customer-impacting | payment, refund | **1 only** + rail | M-of-N + step-up | E3–E5 + settle recon | **Mandatory** | **Forbidden** to approve/execute |

Ghost may *orchestrate* F4/F5 **preparation and evidence**; coordinate-desktop automation must never be marketed as safe F4/F5 execution.

---

## 12. Tamper-evident journal & receipt bundle

### Near-term journal

Append-only events + hash chain (Organizer seal ancestor) + signed checkpoints. Merkle trees / notarization / remote witness = enterprise-later.

### Threat honesty

| Adversary | Residual |
|---|---|
| App bug, bad plugin, bad planner | Mitigated by SoD, tiers, G1–G3 |
| Careless insider | Reduced by UI + SoD; not eliminated |
| Malicious local admin / compromised Ghost + stolen key | **Cannot guarantee G5**; disclose |
| Clock skew | Mitigate NTP awareness; trusted timestamp later |
| DB rollback / log edit | Hash chain detects absent external witness |

### Receipt bundle

```text
transaction.json
plan.ir
policy-decision.json
approval-chain.json
execution-events.ndjson
resource-before.json
resource-after.json
verification.json
reconciliation.json
evidence-manifest.json
outcome-knowledge.json
receipt.sig
```

Embed small hashes + critical fields; reference large evidence blobs; encrypt sensitive fields at rest; redact on export via existing PII mask patterns; external archive optional later.

---

## 13. Trusted execution & attestation (classify)

| Mechanism | Class | Proves | Does not prove |
|---|---|---|---|
| Code signing / notarization / Authenticode | **Required near-term** (shipping honesty) | Publisher identity of bits | Correct execution |
| Platform key stores | Near-term for tokens/receipts | Key protected by OS | Admin cannot extract always |
| Secure Enclave / TPM-backed signing | Enterprise later | Hardware-bound key use | Correct logic |
| Binary measurement / process attestation | Research / enterprise later | Soft integrity claims | External effect truth |
| Confidential computing | Research only | Memory isolation under threat model | Desktop UI truth |
| Reproducible builds / SLSA / SBOM / TUF updates | Near-term→enterprise | Supply-chain provenance | Runtime honesty |
| Signed adapter manifests | Financial-ready | Adapter allowlist | Adapter sincerity of E0 claims |
| Remote attestation of Ghost box | Enterprise later | Device posture | Operator correctness |

---

## 14. Model risk & AI lineage

AI remains outside execution authority. Still record:

```text
model_invocation:
  provider, deployment, model_family, version_claim
  prompt_template_hash, system_policy_hash
  input_resource_refs, output_hash, purpose
  temperature, tools_exposed, data_classification
  human_review_required
```

Non-determinism ⇒ reproducibility **limits** (record seeds/params; do not claim bit-identical). Distinguish: **model proposed · rule derived · human authored · adapter discovered · Ghost verified**.

Model confidence must **not** directly authorize; only via explicit policy mapping to obligations (e.g., low target-identity confidence ⇒ manual confirm). Prompt-injection from documents is in threat model (§19).

---

## 15. Temporal controls

```text
temporal_context:
  observed_time_utc, business_date, organization_timezone
  accounting_period, period_status: open|closed
  cutoff, holiday_calendar_version
```

**Cutoff rule:** Lease and approval carry `valid_until` / period fingerprint. Crossing cutoff or `period_status→closed` is a **material change** ⇒ execution plane refuses until revalidation. Business date ≠ wall clock; DST/holiday calendars are versioned inputs to policy, not hard-coded.

---

## 16. Lineage & classification propagation (narrow)

Track only Ghost-touched transforms needed to authorize/evidence:

`Source → OCR → fields → sheet → dataset → report → email attachment`

Preserve parents, transform, adapter, model involvement, classification, masking, policy decisions, retention, jurisdiction, recipients, purpose. **Confidential+PII does not become Public via summarization.** Field-level tags where cheap; no full data-catalog product.

---

## 17. Human control failure modes

Fatigue, rubber stamps, social engineering, hidden diffs, Unicode spoofing, lookalike recipients, batch concealment, dark patterns, emergency pressure, exception normalization.

**Mitigations:** material-difference views; risk-focused summaries; independent control totals; sampling outliers; per-class UI; domain highlighting; Unicode normalize; chunking large batches; mandatory reason for exceptions; cooldown on F4/F5; approval-quality metrics that audit **control effectiveness**, not individual surveillance scoring.

Human approve ≠ G6.

---

## 18. Formal methods boundary

| Target | Method | Stage |
|---|---|---|
| Transaction + OutcomeKnowledge transitions | Typestate / exhaustiveness + property tests; optional TLA+ for retry/idempotency | Current→Financial-ready |
| Approval invalidation / material change | Property-based + golden vectors | Current |
| Compensation ordering | Model check small saga graphs | Financial-ready |
| Policy obligations | Symbolic/Cedar differential testing | Financial-ready |
| Resource-version invariants | Property tests | Current |
| Audit-chain integrity | Property tests + fixtures | Current |
| Adapter capability isolation | Type/capability boundaries in Rust | Current |
| Full desktop / AX correctness | **Reject** formalization | — |

Proof-carrying *code* of agents: research. Proof *obligations* on actions: Financial-ready.

---

## 19. Operational resilience

Default for F2+ concurrency-affecting / F3+: **fail-closed**. F0 may degrade read-only.

Modes: safe / degraded read-only / recovery / quarantine txs / circuit-break adapters / expired-lease refuse / offline approval only if pre-issued leases exist and policy allows (default **no** for F3+). Identity or policy evaluator unavailable ⇒ no elevating commits. Evidence store unavailable ⇒ do not label Proven; may continue F0.

---

## 20. Financial-services threat model (summary)

| Attack | Mitigate | Residual |
|---|---|---|
| Confused deputy via MCP | Lease bind resources + hashes | Misbound if G0 weak |
| TOCTOU | Version bind + recheck | Semantic rename races |
| Token replay | Single-use + expiry + tx hash | Stolen key |
| Approval substitution | Chain signatures | UI confusion |
| Resource aliasing / Unicode | Normalize + display codepoints | Clever homoglyphs |
| Prompt injection docs | Planner sandbox; no exec authority | Influence of assumptions |
| Model capability escalation | Capability registry allowlist | Social prompt to human |
| Stale policy | Bundle hash on lease | Clock / deployment race |
| Duplicate external submit | Idempotency + OutcomeKnowledge | Peer without keys |
| Evidence laundering / screenshot spoof | Tier+evidence class gates | Determined insider |
| Malicious AX labels | Prefer Tier 0–1 for money | Semantic spoof |
| Break-glass abuse | Dual control + logging | Colluding admins |
| Compromised update | Signing + SLSA/TUF later | Trusted publisher fail |
| Local admin deletes journals | Disclose G5 limit; external archive later | True vs admin |

---

## 21. Claims boundary

### Never claim

- Makes an operation “compliant” or regulatory-approved  
- Guarantees accounting correctness or economic validity  
- Proves human judgment quality  
- Creates atomicity across unrelated systems  
- Guarantees exactly-once external effects without peer support  
- Reverses irreversible actions  
- Secures admin-compromised endpoints  
- Independently validates all third-party adapter claims  
- Replaces internal audit, SoD governance, or core financial systems  
- Makes AI safe because a human clicked approve  
- Treats visual automation as equivalent to API commit  

### Acceptable claims (when implemented)

- Preserves exact approved transaction envelope (G1)  
- Dispatches only actions under valid capability lease (G2∩G3)  
- Records policy decision provenance and evidence actually collected  
- Detects stale resource versions under defined adapters  
- Distinguishes verified / observed / inferred / indeterminate outcomes  
- Produces tamper-evident receipts **within stated threat model**  

---

## 22. Minimal vertical proof (selected wedge)

**Selected:** **Month-end evidence package assembly** (not check-cashing / fraud scoring).

| Criterion | Fit |
|---|---|
| Evidence burden | High (manifest, hashes, dual review) |
| Systems | FS + sheets + optional email/SharePoint later |
| Human approval | Preparer ≠ reviewer |
| Irreversible risk | Low if copy-into-package only (F1→optional F3 send) |
| Time savings | Measurable vs manual folder wrangling |
| Controls | Hash binding, SoD, retention |
| Shadow mode | Build package without send |
| Continuity | Direct extension of Ghost Organizer |

**Skeleton:** Sources in Zones → Class F1 organize/copy → Resource versions + package manifest hash → Control: dual approval on final hash → Receipt + evidence bundle → Optional F3 distribute via Tier ≤1 only → Reconcile recipient/system accept.  
**Manual remains:** Accounting judgments, source completeness, economic sign-off.  
**Why not consulting:** Same transaction machinery as Organizer; package is a productized transaction class, not a bespoke bot.

**Rejected candidates:** Invoice-to-payment (F5 early); funds release; AML/fraud scoring (model-risk center of gravity wrong).

---

## 23. Architecture cut line

Every item appears in **exactly one** stage.

### Current Ghost (build now)

- Canonical transaction envelope (intent, assumptions, reads/writes, actions, hashes)  
- Action IR version field  
- Resource version binding for **local filesystem** (hash/inode/volume)  
- OutcomeKnowledge including Indeterminate / DispatchedUnacknowledged  
- Evidence classes on action results + receipt labels  
- Adapter trust tiers enforced for new surfaces (forbid Tier ≥3 for F3+)  
- Universal receipt schema (fields subset of §12)  
- Material-change invalidation on file hash drift  
- Idempotency keys on Organizer/mutate path  
- Explicit claims boundary in product docs/UI copy  

### Financial-services-ready foundation (before regulated pilot)

- Multi-principal approval + SoD graph  
- Policy decision provenance + signed Cedar (or equiv.) bundles  
- Reconciliation Engine v0 (contracts, exception queue, aging)  
- Temporal context + cutoff/period gates  
- Tamper-evident receipt **bundles** + export lineage  
- Identity integration beyond local vault (enterprise IdP)  
- Stronger Tier-0/1 adapters (Graph/API) for F3  
- Control definition/instance/evidence records  
- Model invocation lineage records  
- Narrow classification/lineage for Ghost transforms  
- Break-glass with dual control  

### Long-term enterprise authority (future)

- Remote attestation / TPM-backed receipt signing as default  
- Multi-organization policy inheritance  
- External witness / notarization service  
- Formal saga orchestration across many SoRs  
- WASM planner ecosystem under Intent Gateway  
- Enterprise retention WORM archives  
- F5 orchestration **only** via payment rails with E3–E5  
- Confidential computing experiments  

---

## 24. Decision log

| Decision | Choice | Failure prevented |
|---|---|---|
| Category | Retain transaction authority; specialize with GCM | Fake ACID marketing |
| VECTRA name | Reject | Brand theater |
| Policy lang | Cedar bundles for FS-ready; Rust now | DIY auth YAML decade |
| UI automation for money | Forbidden as proving path | False settlement confidence |
| Reconciliation | Separate engine | Postcondition ≠ SoR agreement |
| OutcomeKnowledge | First-class Indeterminate | Blind retry double-posts |
| Vertical wedge | Month-end evidence package | Fraud-model distraction |
| GRC scope | Execute/evidence controls only | Product explosion |
| Local admin threat | Disclose residual | False G5 confidence |

---

## 25. Rejected alternatives

- Ghost as core banking / payment switch  
- Ghost as GRC system of record  
- Ghost as AML/fraud scoring product  
- Exact-once guarantees without peer idempotency  
- “ACID-like” unqualified language in marketing  
- Single confidence score for authorization  
- Treating MCP as execution spine  
- Visual Tier-3 as F4/F5 path  
- Fail-open for consequential classes  
- Formal verification of entire desktop agents  

---

## 26. Open research questions

1. Minimal external witness design that resists local admin without becoming cloud-required?  
2. Can UIA/AX evidence reach E2 reliably enough for F2 in Excel-heavy shops?  
3. Practical reconciliation DSL vs per-connector code?  
4. Legal weight of Ghost receipts under common audit standards (as *evidence of control operation*, not compliance certification)?  
5. How to measure approval quality without creating punitive surveillance?  
6. Federated policy overlays across subsidiaries without central execution?  

---

## 27. Final question — smallest truthful architecture today

Build **only**:

1. a **versioned transaction envelope** with bound local file identities;  
2. **OutcomeKnowledge** that admits **Indeterminate** and forbids blind retry;  
3. **evidence-classified receipts** that refuse a single green checkmark;  
4. **adapter trust tiers** that ban visual automation as proof for consequential classes;  
5. **material-change invalidation**;  

…and **say out loud** that Ghost does not yet provide bank-grade G2 across enterprise IdPs, G4 across SaaS without APIs, G5 against administrators, or any G6 business correctness.

That stack is smaller than the full design, continuous with Organizer, and uncomfortable in the places truth is uncomfortable — which is exactly the point.
