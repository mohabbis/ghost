# Ghost Next-Generation Architecture

> **Architectural north star:** For five-year KEEP / REFACTOR / REPLACE / REMOVE decisions, platform ownership, Action IR, plugin runtime, and product NEVER-boundaries, see [`architecture-review-2031.md`](architecture-review-2031.md). Where this document’s ambition (marketplace, remote MCP, “OS for safe AI actions”) conflicts with that review or with the `AGENTS.md` wedge build order, prefer the architecture review and `AGENTS.md`. This file remains the MCP / Zones / approval-token vision sketch.

Ghost's long-term architecture is the trusted execution layer between AI reasoning systems and a user's local digital workspace. AI systems reason, Ghost verifies, the user approves, and deterministic code executes.

Ghost is not another AI assistant. Ghost is the operating system for safe AI actions: a vendor-neutral execution engine where every filesystem operation is planned, reviewed, approved, audited, and reversible.

## Product mission

Build Ghost into the universal execution engine that allows any AI client to safely interact with user files through deterministic planning and policy enforcement.

The durable product analogy is:

```text
Git for AI file operations
```

Every operation should be:

- planned;
- reviewed;
- approved;
- executed deterministically;
- fully audited;
- reversible.

The model never directly manipulates the filesystem. Ghost always owns execution.

## Core principles

### 1. AI never executes

Models produce intent and structured suggestions. Ghost produces executable plans and performs execution.

Do not allow AI clients or provider outputs to introduce:

- arbitrary shell execution;
- generated scripts;
- model-generated filesystem commands;
- raw code execution.

Models may suggest outcomes such as `Move screenshots into monthly folders`. Ghost converts suggestions into deterministic operations, validates them against policy, previews them, and executes only after approval.

### 2. One integration layer

Do not build one-off execution integrations for ChatGPT, Claude, Gemini, Codex, Cursor, Windsurf, VS Code, or future providers. Build one protocol surface and make every client use it.

That protocol surface is MCP:

```text
AI clients
  ChatGPT / Claude / Codex / Cursor / future clients
        |
        v
Ghost MCP server
        |
Planning -> Policy -> Approval -> Audit -> Undo
        |
Deterministic engine
        |
Local filesystem
```

Ghost owns execution. The model and client do not.

### 3. Local first

Files remain local by default. Cloud and remote connectivity are optional capabilities, not architectural requirements.

- Metadata sharing is explicit.
- Content sharing is explicit.
- Nothing leaves the machine without consent.

### 4. Explainability

Every plan must answer:

- Why is this plan being proposed?
- What exact operations will happen?
- How will Ghost execute them?
- What are the risks and conflicts?
- How can the run be undone?

There should be no hidden decisions in the execution path.

### 5. Approval is sacred

No AI client may approve a plan, modify an approved plan, reuse approval, or escalate permissions. Approval exists only inside Ghost.

Approved execution requires an approval artifact bound to the plan and scope that the user reviewed.

## Primary architecture

```text
AI client
  |
MCP request
  |
Authentication layer
  |
Planning request schema
  |
Deterministic planner
  |
Policy engine
  |
Conflict detection
  |
Preview generation
  |
User approval
  |
Signed approval token
  |
Execution engine
  |
Audit + undo journal
```

## Major components

### MCP server

Expose a stable tool surface. Do not expose raw filesystem APIs.

Allowed tool families:

- read;
- scan;
- plan;
- validate;
- explain;
- approval request/status;
- execute approved plans;
- undo completed runs.

### Deterministic planner

The planner is responsible for:

- path resolution;
- conflict detection;
- rename planning;
- move planning;
- rule generation;
- dependency ordering;
- rollback generation.

Models never perform final planning.

### Policy engine

The policy engine is responsible for:

- Zone enforcement;
- forbidden locations;
- permissions;
- overwrite prevention;
- protected files;
- system boundaries;
- organization rules.

The policy engine must evaluate immediately before execution, even if the plan was already validated earlier.

### Approval manager

The approval manager produces approval tokens bound to a reviewed plan.

Each token contains:

- plan hash;
- capabilities;
- expiry;
- single-use flag;
- Zone scope;
- execution permissions.

Execution fails if the plan, scope, files, or policy-relevant facts change in a way that invalidates approval.

### Execution engine

The execution engine performs only deterministic operations. It never interprets AI output, evaluates prompts, or executes arbitrary code.

### Audit journal

Every operation produces durable records containing:

- before state;
- after state;
- timestamps;
- plan reference;
- user or session reference;
- approval reference;
- undo information.

### Undo engine

Undo is first-class. Every supported mutation should have an undo plan or journal entry before execution begins.

## MCP tool surface

Suggested MCP tools should remain plan-oriented rather than filesystem-oriented.

### Read tools

- `ghost_get_status`
- `ghost_list_zones`
- `ghost_scan_zone`
- `ghost_get_run`
- `ghost_get_plan`
- `ghost_list_workflows`
- `ghost_get_audit_log`

### Planning tools

- `ghost_create_plan`
- `ghost_validate_plan`
- `ghost_explain_plan`
- `ghost_explain_policy`
- `ghost_estimate_changes`

### Approval tools

- `ghost_request_approval`
- `ghost_get_approval_status`
- `ghost_cancel_plan`

### Execution tools

- `ghost_execute_plan`
- `ghost_undo_run`
- `ghost_verify_run`

## Remote architecture

### Local MCP

Local integrations should support desktop and developer clients through local transports such as:

- stdio;
- named pipes;
- domain sockets.

### Remote MCP

Remote connectivity may use a secure tunnel or Ghost relay:

```text
AI client
  |
Ghost cloud relay
  |
Encrypted tunnel
  |
Ghost desktop
  |
Execution
```

The relay never executes plans and never stores file contents. It only routes authenticated requests.

## Security model

Every request must flow through:

```text
Authentication
  -> Authorization
  -> Policy
  -> Planning
  -> Preview
  -> Approval
  -> Execution
  -> Audit
```

There are no bypasses for privileged clients, plugins, or model providers.

## Provider abstraction

Ghost should never depend on one model provider. Define an internal `IntelligenceProvider` boundary for suggestion-only intelligence.

Provider implementations may include:

- OpenAI;
- Anthropic;
- Google;
- OpenRouter;
- Ollama;
- LM Studio;
- vLLM;
- Disabled.

Providers return structured schemas only. They never return executable commands.

The intelligence pipeline is:

```text
Prompt
  -> Provider
  -> Structured suggestion
  -> Validation
  -> Deterministic planner
  -> Policy
  -> Approval
  -> Execution
```

## Zones

Everything operates inside Zones. Zones define:

- scope;
- permissions;
- organization rules;
- allowed mutations;
- protected paths.

All execution occurs inside a Zone.

## Plugin architecture

Future integrations should register planning capabilities, not execution authority.

Examples:

- `ghost-photo-plugin`
- `ghost-git-plugin`
- `ghost-media-plugin`
- `ghost-backup-plugin`
- `ghost-cloud-plugin`

Plugins expose planning primitives. Execution still belongs to Ghost.

## Data model

Primary entities:

- Workspace;
- Zone;
- Policy;
- Plan;
- Operation;
- Approval;
- Execution;
- AuditEntry;
- UndoJournal;
- Workflow;
- Provider;
- Device;
- Session;
- Pairing;
- Capability;
- Conflict.

Every mutation references a Plan. Every Plan references an Approval before execution. Every executed run references Audit and Undo records.

## Roadmap

1. Core planner, policy engine, execution engine, audit, undo, and Zone system.
2. Local MCP server with read-only tools, planning tools, and desktop approval UI.
3. Execution tokens, approval integrity, signed plans, and conflict revalidation.
4. Remote connectivity with secure device pairing, desktop tunnel, Ghost relay, remote authentication, and session management.
5. Provider abstraction for OpenAI, Anthropic, Gemini, and local models with prompt redaction and metadata minimization.
6. Workflow marketplace, plugin SDK, community extensions, enterprise policy packs, and multi-device orchestration.

## Success criteria

Ghost succeeds when:

- AI clients integrate once through MCP instead of bespoke APIs.
- Every filesystem mutation is deterministic, reviewable, and reversible.
- Users can trust AI-assisted actions without surrendering control.
- The platform remains vendor-neutral while Ghost remains the authoritative execution engine.
- Third-party developers can extend Ghost through plugins and workflow packs without compromising the security model.

The guiding principle for every architectural decision is:

```text
Reasoning is replaceable. Execution is not. Ghost owns execution.
```
