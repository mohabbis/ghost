# Enterprise Financial Operations

Ghost may support financial-operations workflows only when they preserve the trust pipeline:

```text
Intent -> Plan -> Policy check -> User approval -> Deterministic execution -> Audit log -> Undo path
```

Enterprise features must remain local-first, explainable, auditable, reversible where practical, and explicitly controlled by authorized users. Ghost must not become hidden surveillance, an uncontrolled autonomous agent, or a black-box financial decision maker.

## Initial architecture

The enterprise architecture is split into narrow Rust module boundaries:

- `enterprise`: playbooks, workflow runs, approvals, exceptions, reports, scheduling, and tenancy boundaries.
- `finance`: reconciliation, invoices, daily reports, close support, and controls.
- `checks`: check metadata extraction, validation, duplicate detection, risk signals, and review packets.
- `fraud`: rule/scoring/evidence/model/evaluation boundaries for decision support only.
- `compliance`: KYC, AML, retention, audit-package, and controls support.
- `data_protection`: encryption, redaction, secrets, retention, and secure-delete boundaries.

These modules are scaffolding and domain models only. They do not add Tauri commands, background monitoring, network calls, or financial mutations.

## Playbooks

A playbook defines human-readable operating limits:

- workflow name and business purpose;
- approved data sources, folders, file types, and applications;
- allowed and prohibited actions;
- required approval levels;
- exception thresholds;
- retention, audit, and undo requirements;
- rule version.

The UI should expose only three simple trust states:

| UI state | Internal policy |
|---|---|
| Run after review | `allow_after_review` |
| Ask every time | `ask_first` |
| Never | `never` |

AI may draft playbooks, labels, explanations, or suggested mappings. AI output must never directly authorize execution.

## Workflow runtime

Enterprise workflow runs use explicit lifecycle states: draft, planning, awaiting approval, approved, executing, paused, awaiting review, completed, partially completed, failed, cancelled, and rolled back.

Every run records the initiating user, approved inputs, proposed actions, policy decisions, approvals, execution results, exceptions, audit entries, and an optional undo manifest. Every mutating action requires a non-empty idempotency key so retry and recovery cannot duplicate outputs or apply the same move twice.

## Financial safety limits

Ghost may prepare, classify, compare, reconcile, and recommend. Ghost must not independently:

- approve or deny a check-cashing transaction;
- post a journal entry;
- release payment or transmit funds;
- change bank information;
- approve an invoice;
- modify a closed accounting period;
- silently alter original financial reports;
- overwrite or delete financial records.

Original financial files must be preserved or copied into an immutable source area before transformations occur.

## Fraud and compliance limits

Fraud and compliance outputs are decision support only. Risk results must include score, contributing signals, source data, rule/model version, confidence, recommended reviewer action, limitations, timestamp, and reviewer outcome before any production promotion.

Ghost must not claim to determine legal or regulatory compliance automatically. It may help authorized employees collect evidence, apply configured rules, and document human decisions.
