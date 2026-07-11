# Ghost Project — Detailed Handoff Prompt for Continued Development

**Last Updated:** 2026-07-11
**Status:** `master` green. **v1.2.5 released** (`Ghost.dmg` + `Ghost_Setup.exe`). Professional polish in progress: atomic release publish + checksums, marketing honesty, app error/version UX.

---

## What You're Picking Up

You are inheriting **Ghost**, a Tauri (Rust + vanilla JS) local-first desktop automation product for macOS and Windows. Read `AGENTS.md` and `CLAUDE.md` first — they are the canonical contract. The product promise is trustworthy execution:

```text
Record -> Inspect -> Approve -> Replay -> Audit -> Undo
```

The current wedge is **Ghost Organizer** (safe file organization: scan → plan → review → approve → move/rename → audit → undo), fully wired end to end through the policy engine, executor, audit log, and undo journal.

---

## Recent Changes (through v1.2.4 + polish)

1. **v1.2.4:** ID-scan OCR hardening, DOM contract test, signing-docs fix, version bump.
2. **Release pipeline:** single publish job after both platform builds; `SHA256SUMS.txt`; optional updater artifacts / `latest.json` when keys exist; no more Mac-only or Windows-only partial releases.
3. **Product polish:** platform-neutral copy, honest signing/preview language, readable IPC error toasts, Settings shows app version.

---

## Current Health

- Rust CI + Deploy Website on `master`.
- Validation: `make ci` (fmt + clippy + test), `make build`, `make dev`.
- Local Linux needs GTK/webkit deps in `AGENTS.md` plus `libssl-dev` / `pkg-config`.

---

## Immediate Next Steps

1. **Notarization / Windows signing secrets** so releases stop being ad-hoc/unsigned. See `RELEASING.md`.
2. **Updater pubkey:** replace `REPLACE_WITH_…` in `tauri.conf.json` and set `TAURI_SIGNING_PRIVATE_KEY` so `latest.json` publishes automatically.
3. **Keep Guard Desk / POS Bridge suggestion-only** — never auto-execute from ID-scan output.
4. Continue `AGENTS.md` build order: Organizer polish → replay reliability → release quality → AI last (gated).

---

## Known Risks

- Without Apple/Azure secrets, releases are preview-quality (ad-hoc macOS / unsigned Windows).
- `parse_id_document` handles PII — keep local and suggestion-only.
- Experimental commands stay behind `--features experimental`; CI does not run that leg.

---

## 6. Microsoft Enterprise Integration Layer

Ghost must integrate cleanly with the Microsoft enterprise ecosystem without becoming dependent on a single vendor.

Support Microsoft products through a dedicated connector architecture.

Recommended modules:

```text
src-tauri/src/connectors/
├── mod.rs
├── microsoft/
│   ├── mod.rs
│   ├── identity/
│   ├── graph/
│   ├── sharepoint/
│   ├── onedrive/
│   ├── teams/
│   ├── outlook/
│   ├── power_bi/
│   ├── fabric/
│   ├── sql_server/
│   ├── azure_storage/
│   └── key_vault/
├── connector_policy.rs
├── connector_registry.rs
├── credentials.rs
├── health.rs
└── audit.rs
```

Each connector must expose a common interface:

```rust
pub trait EnterpriseConnector {
    fn connector_id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn capabilities(&self) -> Vec<ConnectorCapability>;
    fn connection_status(&self) -> ConnectorStatus;
    fn validate_configuration(&self) -> Result<(), ConnectorError>;
    fn test_connection(&self) -> Result<ConnectorHealth, ConnectorError>;
    fn execute_read(
        &self,
        request: ReadRequest,
        context: ExecutionContext,
    ) -> Result<ConnectorResponse, ConnectorError>;
    fn propose_write(
        &self,
        request: WriteRequest,
        context: ExecutionContext,
    ) -> Result<PlannedConnectorAction, ConnectorError>;
}
```

Connector writes must never execute directly from untrusted model output.

Every write must pass through:

```text
Request
→ Connector capability check
→ Tenant policy check
→ Data classification check
→ Plan generation
→ Human approval
→ Credential authorization
→ Deterministic execution
→ Audit record
```

---

## 7. Microsoft Identity and Authentication

Use Microsoft Entra ID for enterprise authentication.

Support:

- OAuth 2.0
- OpenID Connect
- Microsoft Authentication Library patterns
- delegated user permissions
- application permissions
- service principals
- managed identities where available
- certificate-based authentication
- conditional access compatibility
- multifactor authentication compatibility
- tenant restrictions
- role-based access control

Do not store Microsoft passwords.

Do not automate login by recording credentials or replaying keystrokes.

Store tokens only through operating-system-backed secure storage:

- macOS Keychain
- Windows Credential Manager or DPAPI
- Azure Key Vault for enterprise-managed deployments

Tokens must:

- be encrypted at rest
- have explicit scopes
- be revocable
- never appear in normal logs
- never appear in crash reports
- never be sent to analytics
- be redacted from diagnostics
- be isolated by tenant and connector

Define:

```rust
pub struct EnterpriseIdentity {
    pub tenant_id: String,
    pub principal_id: String,
    pub principal_type: PrincipalType,
    pub display_name: String,
    pub assigned_roles: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub token_reference: SecretReference,
}
```

Require least-privilege permissions.

Ghost must clearly show administrators which permissions each connector requires and why.

---

## 8. Microsoft Graph Integration

Use Microsoft Graph for approved access to:

- Outlook
- Teams
- SharePoint
- OneDrive
- Microsoft 365 users and groups
- approved files
- calendar metadata
- enterprise notifications

Potential workflows include:

- retrieve approved daily report attachments from a shared mailbox
- detect missing branch submissions
- save approved files into SharePoint
- notify a reviewer through Teams
- create a draft email requesting a missing report
- read files from an approved OneDrive folder
- add a task to an approved operational queue

Default all Graph permissions to read-only.

Write capabilities must be separately enabled.

The following actions require explicit approval:

- sending an email
- posting a Teams message
- uploading or replacing a file
- changing permissions
- moving or deleting cloud files
- editing calendar events
- modifying SharePoint lists

Ghost may create drafts automatically when a playbook allows it, but sending must remain separately approved unless an administrator explicitly configures a narrowly scoped trusted workflow.

---

## 9. SharePoint and OneDrive

Support SharePoint and OneDrive as governed document sources and destinations.

Capabilities should include:

- list files in approved sites and drives
- download approved files
- read file metadata
- detect new or changed files
- upload generated reports
- create versioned output
- write processing status to approved metadata fields
- preserve original documents
- check file hashes
- prevent duplicate ingestion
- enforce retention labels when available

Never overwrite an existing enterprise document by default.

Use versioned names or SharePoint versioning.

Every cloud file action must capture:

- tenant
- site
- drive
- file ID
- file version
- content hash
- action
- actor
- approval
- timestamp
- resulting location

---

## 10. Microsoft SQL Server Integration

Support:

- Microsoft SQL Server
- Azure SQL Database
- SQL Server through approved on-premises gateways
- read replicas
- stored procedures
- parameterized queries
- integrated authentication
- Entra authentication
- certificate-based connections

Do not execute dynamically generated raw SQL from AI output.

All queries must come from:

- administrator-approved query templates
- stored procedures
- parameterized commands
- version-controlled report definitions

Create a query-template model:

```rust
pub struct ApprovedQueryTemplate {
    pub query_id: String,
    pub name: String,
    pub version: String,
    pub connection_id: String,
    pub statement: String,
    pub allowed_parameters: Vec<QueryParameterDefinition>,
    pub read_only: bool,
    pub maximum_rows: u64,
    pub timeout_seconds: u64,
    pub permitted_roles: Vec<String>,
}
```

Enforce:

- parameterization
- timeouts
- row limits
- read-only transactions by default
- connection pooling
- cancellation
- query audit logs
- schema validation
- sensitive-column classification
- field-level redaction
- no credentials in source code

Write operations must use approved stored procedures and require elevated approval.

Ghost should initially focus on SQL Server read workflows:

- retrieve daily branch totals
- compare reports against source systems
- validate transaction counts
- identify missing records
- generate exception lists
- populate consolidated reports
- supply governed datasets to Power BI or Fabric

---

## 11. Microsoft Fabric Integration

Treat Microsoft Fabric as the enterprise analytics and data platform.

Support integration with:

- Fabric Lakehouse
- Fabric Warehouse
- OneLake
- Data Factory pipelines
- notebooks
- semantic models
- event streams where applicable
- deployment pipelines
- Fabric APIs
- approved Power BI datasets

Ghost should not attempt to replace Fabric.

Ghost should serve as:

- a secure desktop and branch-edge intake layer
- a human-review and exception layer
- a local workflow execution layer
- a governed source of operational files
- a trigger and monitoring client for approved Fabric processes

Example flow:

```text
Branch report arrives locally
→ Ghost validates and hashes source
→ Ghost extracts approved fields
→ User reviews exceptions
→ Ghost uploads approved package to OneLake
→ Fabric pipeline processes data
→ Fabric semantic model refreshes
→ Power BI dashboard updates
→ Ghost records pipeline and refresh status
```

Support Fabric job monitoring:

- submitted
- queued
- running
- succeeded
- failed
- cancelled

Display failures in plain language.

Do not expose raw cloud error objects to ordinary users.

Allow administrators to configure:

- workspace
- lakehouse
- warehouse
- pipeline
- semantic model
- destination path
- retry policy
- approval policy
- retention policy

---

## 12. Power BI Integration

Support Power BI as the reporting and management-consumption layer.

Capabilities should include:

- trigger approved dataset refreshes
- monitor refresh status
- retrieve refresh history
- verify report freshness
- publish approved report exports
- open relevant dashboards
- capture report metadata
- notify managers when a refresh fails
- compare operational totals against published dashboard totals

Ghost must not silently publish or replace Power BI content.

Initial Power BI support should focus on:

1. data preparation
2. refresh orchestration
3. refresh monitoring
4. discrepancy detection
5. management notification

Example daily workflow:

```text
Collect daily branch files
→ Validate all expected branches
→ Reconcile totals against SQL Server
→ Produce exception queue
→ Obtain approval
→ Deliver validated data to Fabric
→ Trigger Power BI refresh
→ Confirm completion
→ Produce daily control report
```

The daily control report should show:

- expected branches
- received branches
- missing branches
- duplicate submissions
- transaction count variance
- dollar variance
- rejected records
- unresolved exceptions
- Fabric pipeline status
- Power BI refresh status
- reviewer
- approval timestamp

---

## 13. Microsoft Teams Integration

Use Teams for operational alerts and review requests, not uncontrolled chat automation.

Ghost may send structured approval cards containing:

- workflow name
- business date
- summary
- exception count
- risk level
- proposed actions
- approve link
- reject link
- open-in-Ghost link

Sensitive information must be minimized.

Do not place full account numbers, government IDs, check images, customer documents, or unnecessary personal data in Teams messages.

Use masked values:

```text
Account: ****4821
Customer ID: CUS-10482
Check: ****7714
```

Teams approval should create a signed approval record tied to the authenticated Entra user.

---

## 14. Outlook and Shared Mailbox Integration

Support approved shared mailboxes for workflows such as:

- branch report collection
- invoice intake
- missing-document tracking
- compliance correspondence
- management report delivery

Ghost should:

- inspect only explicitly approved mailboxes and folders
- filter by configured senders, subjects, dates, and attachment types
- download approved attachments
- hash attachments
- detect duplicates
- prepare draft replies
- route suspicious messages for review

Ghost must not broadly read a user’s entire mailbox.

Email access must be scoped to the minimum necessary mailbox, folder, or query.

---

## 15. Enterprise Data Loss Prevention

Implement a data-classification layer.

Suggested classifications:

```rust
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Financial,
    PersonallyIdentifiable,
    HighlyRestricted,
}
```

Highly restricted data includes:

- full Social Security numbers
- full bank account numbers
- authentication secrets
- unmasked payment card information
- check images
- government identification images
- biometric data

Before any connector sends data outside the local machine, Ghost must run:

```text
Data classification
→ Destination policy check
→ Redaction or minimization
→ Approval requirement
→ Transfer
→ Audit
```

Add configurable rules such as:

- check images cannot be sent to Teams
- full account numbers cannot appear in Power BI exports
- raw IDs cannot leave approved encrypted storage
- customer PII cannot be included in telemetry
- SQL query results containing restricted columns cannot be exported without approval

---

## 16. Fraud Detection Architecture

Fraud detection must combine deterministic rules with optional approved models.

Use separate layers:

```text
Document validation
→ Duplicate detection
→ Historical comparison
→ Rule-based risk signals
→ Optional model scoring
→ Evidence aggregation
→ Human review
```

Potential risk signals include:

- duplicate check number
- duplicate image hash
- duplicate amount and payer combination
- abnormal transaction frequency
- customer velocity increase
- unusual branch activity
- amount outside customer history
- altered check fields
- inconsistent fonts or alignment
- invalid date
- stale-dated check
- routing-number mismatch
- payee mismatch
- account history discrepancy
- repeated declined item
- geographic inconsistency

Each signal must be independently explainable.

Example:

```rust
pub struct FraudSignal {
    pub signal_id: String,
    pub signal_type: FraudSignalType,
    pub severity: RiskSeverity,
    pub description: String,
    pub evidence_references: Vec<String>,
    pub rule_version: String,
    pub confidence: Option<f64>,
}
```

Never use protected characteristics or obvious proxies for protected characteristics.

Fraud models must be evaluated for:

- false-positive rate
- false-negative rate
- precision
- recall
- calibration
- drift
- branch-level variation
- customer-segment variation
- reviewer override rate

Model output must never be the sole basis for denying a transaction.

---

## 17. Security Requirements

Implement enterprise controls including:

- encryption at rest
- encryption in transit
- tenant isolation
- role-based access control
- least privilege
- secret rotation
- certificate rotation
- signed releases
- notarized macOS builds
- signed Windows installers
- signed updater artifacts
- tamper-evident audit logs
- local data retention controls
- secure export
- session timeout
- device trust
- admin policy locking
- connector allowlists
- network destination allowlists
- diagnostic redaction

No sensitive data may be sent to third-party AI services by default.

Any AI provider must be explicitly configured by an administrator.

Support these deployment modes:

1. Fully local
2. Local with Microsoft enterprise connectors
3. Azure-hosted private model endpoint
4. Enterprise-approved external model with redaction
5. No-AI deterministic mode

---

## 18. User Experience

The application must remain simple enough for branch and accounting employees.

Primary navigation:

- Home
- Work Queue
- Daily Reports
- Checks
- Reconciliations
- Exceptions
- Playbooks
- Audit
- Integrations
- Settings

Home should show:

- work waiting for review
- failed workflows
- missing reports
- unresolved discrepancies
- recent completed work
- system health
- connector health

Do not expose technical architecture to standard users.

Use plain-language statuses:

- Ready
- Waiting for approval
- Processing
- Needs review
- Completed
- Could not complete
- Reversed

The application may live in the menu bar or system tray and remain unobtrusive, but it must always provide:

- visible active-work indicator
- pause control
- stop control
- open-workflow control
- privacy state
- connector state

---

## 19. Implementation Order

Do not build every integration simultaneously.

Implement in this order:

### Phase 1

- connector registry
- Entra ID authentication
- secure credential storage
- SQL Server read connector
- SharePoint and OneDrive read connector
- Microsoft Graph read connector
- connector audit logs
- connector health checks
- data classification

### Phase 2

- daily branch reporting workflow
- report ingestion
- duplicate detection
- SQL reconciliation
- exception queue
- consolidated report generation
- SharePoint output
- Teams review notification

### Phase 3

- Fabric upload and pipeline monitoring
- Power BI refresh orchestration
- daily control report
- operational dashboard integration

### Phase 4

- Outlook shared mailbox intake
- invoice workflows
- reconciliation workflows
- compliance package workflows

### Phase 5

- check-document extraction
- deterministic fraud rules
- duplicate-check detection
- explainable risk review
- controlled private-model support

---

## 20. Non-Negotiable Requirements

Do not:

- bypass Ghost’s approval pipeline
- silently record screens
- silently record keyboard input
- upload data without policy approval
- store plaintext secrets
- execute AI-generated SQL
- use AI output as direct execution instructions
- overwrite source financial records
- autonomously approve or deny financial transactions
- transmit funds
- approve invoices
- post journal entries without authorization
- claim regulatory compliance automatically
- expose PII in logs or notifications
- place unrestricted customer data in Power BI
- broadly read Outlook mailboxes
- make connectors request excessive permissions

---

## 21. Deliverables

Produce:

1. Architecture assessment of the existing repository
2. Gap analysis against this specification
3. Detailed implementation plan
4. Connector interface and registry
5. Entra ID authentication design
6. SQL Server read-only connector
7. SharePoint and OneDrive connector
8. Microsoft Graph connector
9. Fabric and Power BI integration specifications
10. Daily branch report prototype
11. Exception review interface
12. Data-classification and redaction engine
13. Threat model
14. Security tests
15. Integration tests
16. Migration documentation
17. Administrator deployment guide
18. User-facing workflow guide
19. Updated command registry
20. Updated product-boundary documentation

Do not claim features are complete unless they are implemented and tested.

Preserve current stable behavior.

Keep experimental functionality behind feature flags.

The finished system should demonstrate one complete enterprise workflow before expanding:

```text
Receive branch reports
→ Validate submissions
→ Reconcile against SQL Server
→ Review discrepancies
→ Upload approved data to Fabric
→ Refresh Power BI
→ Produce audit-ready daily report
```

That workflow is the acceptance target.
