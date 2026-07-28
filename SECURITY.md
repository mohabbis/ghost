# Security Policy

Ghost Cloud is an approval-gated AI operator. Security is part of the product
promise: least privilege, human approval on sensitive actions, verification, and
tamper-evident audit logs.

## Principles

1. **Org isolation** — every record is scoped to an organization.
2. **Deny-by-default** — send / pay / delete / submit halt for approval.
3. **Deterministic gates** — models do not decide sensitivity or execute plans.
4. **No secret sprawl** — credentials and PII stay out of logs, screenshots, and prompts.
5. **Auditability** — runs append to a hash-chained log.

## Reporting a vulnerability

Email the maintainer via GitHub
([@mohabbis](https://github.com/mohabbis)) with a private advisory if possible.
Please include reproduction steps and impact. Do not open a public issue for
exploitable flaws.

We aim to acknowledge within 72 hours.

## Scope

In scope: `cloud/` (web, worker, core), auth, run execution, approval flow, audit.

Out of scope for new feature work: the legacy desktop app’s network surface,
except critical remotely exploitable issues in published installers.
