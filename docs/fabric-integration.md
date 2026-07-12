# Fabric Integration

Status: **planned** (module boundaries **partially built**)

## Product rule

Fabric is a **business-system connector** (Layer B), not an intelligence provider. Fabric pipelines must not trigger desktop mutations without Ghost's full approval pipeline.

## First use case (planned)

```text
Ghost audit history
  -> user selects date range
  -> structured summary
  -> user reviews export preview
  -> user approves
  -> Ghost writes to selected Fabric destination
  -> local audit entry
```

## Module layout — **partially built**

```text
src-tauri/src/integrations/microsoft/
├── fabric/
│   └── mod.rs        FabricClient stub
├── scopes.rs         fabric scope placeholders
└── mod.rs            grant check: identity alone ≠ Fabric access
```

## Phase-one capabilities — **planned**

- List accessible workspaces (read-only)
- Inspect workspace metadata
- List selected Fabric items
- Validate workspace access
- Export approved audit summaries
- Persist selected workspace as scoped setting
- Audit every outbound export locally

## Grant requirement — **built**

`MicrosoftIntegrationService::fabric_grant_active` returns `ConsentRequired` when only an identity grant exists.

## Not in scope (phase one)

- Inbound Fabric-triggered file mutations
- Notebook/pipeline direct desktop control
- Silent export

See `docs/power-bi-integration.md` for related Power BI export shape.
