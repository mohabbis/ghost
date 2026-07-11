# Ghost Post-Audit Implementation Handoff

Use this handoff after the comprehensive audit report is complete. The audit report is the single source of truth for scope, severity, evidence, and acceptance criteria. Treat every audit finding as validated unless the report explicitly marks it as tentative or informational.

## Role

You are joining the Ghost project as the lead software architect and senior engineer.

Your objective is to move Ghost from its current audited state toward a production-ready desktop application while preserving the product contract:

- local-first behavior;
- explicit user approval;
- deterministic execution;
- complete auditability;
- safe undo;
- no silent file mutations;
- privacy by default.

Do not add new features until the platform is stable unless the audit roadmap explicitly requires the work.

## Primary goals

1. Eliminate every Critical and High severity finding.
2. Bring the website, repository, documentation, and releases into alignment.
3. Increase confidence through automated testing and release verification.
4. Deliver a production-ready v2 release candidate only after the release-readiness criteria are met.

## Engineering priorities

Work in this order.

### Phase 1 — Product integrity

Resolve all discrepancies between:

- website;
- GitHub repository;
- releases;
- download artifacts;
- documentation;
- version numbers.

Every public claim must be backed by working implementation or moved to the roadmap.

### Phase 2 — Security

Implement every Critical and High severity recommendation.

Focus especially on:

- IPC validation;
- filesystem boundaries;
- replay safety;
- path traversal prevention;
- undo durability;
- privileged command isolation;
- native permission handling;
- secret management;
- release signing;
- supply-chain hardening.

Every privileged action must be testable.

### Phase 3 — Architecture cleanup

Refactor the project into clearly separated modules with explicit ownership boundaries, including:

- UI;
- application state;
- IPC;
- workflow engine;
- planner;
- validator;
- executor;
- undo manager;
- audit logger;
- recording engine;
- replay engine;
- target resolver;
- native adapters;
- persistence;
- AI services;
- configuration;
- update manager.

Avoid large files with mixed responsibilities.

### Phase 4 — Reliability

Improve:

- error handling;
- recovery;
- logging;
- crash resilience;
- file transaction safety;
- cross-platform consistency.

All destructive operations must be resumable or safely recoverable.

### Phase 5 — Testing

Create automated coverage for:

- Rust units;
- integration tests;
- IPC;
- replay;
- undo;
- filesystem mutations;
- planner;
- validator;
- AI safety boundaries;
- web UI;
- installer smoke tests.

Every bug fixed from the audit must receive a regression test before the finding is marked resolved.

### Phase 6 — Documentation

Update the following documents so they match implementation exactly:

- README;
- architecture;
- developer setup;
- security model;
- privacy model;
- release process;
- contribution guide;
- threat model;
- testing guide;
- versioning policy;
- deployment guide.

### Phase 7 — Website

Make the website an accurate reflection of the application. Review:

- landing page;
- downloads;
- feature matrix;
- roadmap;
- screenshots;
- demos;
- FAQs;
- installation;
- trust model.

Remove marketing language that overstates implementation.

### Phase 8 — Release engineering

Modernize releases so the release process can produce:

- reproducible builds;
- signed binaries;
- notarized macOS builds;
- Windows signed installers;
- SHA-256 manifests;
- changelog generation;
- automated release notes;
- CI verification;
- release promotion workflow.

No release should be manual beyond approval.

## Required deliverables

### 1. Updated architecture

Provide:

- PlantUML diagrams;
- component boundaries;
- trust boundaries;
- data flow;
- IPC map;
- replay state machine;
- filesystem transaction lifecycle.

### 2. Refactored source

Deliver:

- organized modules;
- reduced technical debt;
- clear ownership boundaries.

### 3. Updated website

Deliver:

- consistent branding;
- accurate feature descriptions;
- verified downloads;
- correct versioning.

### 4. Automated CI/CD

Deliver:

- GitHub Actions coverage for linting, tests, security scans, release validation, and artifact verification.

### 5. Security improvements

Deliver:

- threat model;
- risk register;
- mitigation tracking;
- verification checklist.

### 6. Production readiness report

Summarize:

- completed findings;
- deferred findings;
- known limitations;
- remaining risks;
- release recommendation.

## Working rules

- Work from highest-risk findings first.
- Make incremental commits.
- Keep pull requests focused.
- Preserve backwards compatibility unless a breaking change is justified.
- Add tests before marking any finding as resolved.
- Do not close an audit item without evidence.
- Update documentation whenever behavior changes.
- Never implement a feature solely to satisfy marketing copy.

## Definition of done

The implementation stage is complete when:

- no Critical findings remain;
- no High findings remain;
- website and repository are consistent;
- releases are reproducible;
- security recommendations are implemented;
- automated tests pass;
- documentation matches implementation;
- macOS and Windows builds succeed;
- all download artifacts are signed and verified;
- the project can be handed to a new engineer with minimal onboarding.

Every completed audit item must reference the commit or commits, pull request or pull requests, tests, and documentation updates that resolved it, providing a fully traceable path from finding to verification.

## Three-stage workflow

1. **Audit Prompt** — produces the comprehensive audit report.
2. **Implementation Handoff** — executes and resolves the audit findings.
3. **Final Verification & Release Handoff** — independently validates the fixes, performs regression testing, and certifies the release before publication.

The final stage exists to verify that implementation actually addressed the audit instead of simply claiming completion.
