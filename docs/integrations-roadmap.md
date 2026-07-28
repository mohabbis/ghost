# Integrations roadmap (cloud)

Connectors exist to **execute approved plans** against systems customers already
use. They never bypass approval or audit.

## Order

1. Finish recording → editable steps (Phase 2)
2. First connectors with clear ROI for the ICP:
   - Email (Gmail / Outlook) — read + draft/send with approval on send
   - Cloud files (Drive / SharePoint) — scoped read/write
   - CRM / ERP (HubSpot, Salesforce, QuickBooks, NetSuite-class) — one vertical at a time
3. Desktop automation only when API + browser are insufficient

## Rules for every connector

- Scoped, revocable OAuth (or equivalent); disclose scopes in UI
- Secrets encrypted at rest; never in audit payloads or screenshots
- Mutating operations classified; send/pay/delete/submit require approval
- Every call appears on the run timeline + audit chain

## Explicitly deferred

- Desktop Fabric / Power BI export experiments (legacy)
- MCP relay as a product surface
- Broad “connect anything” marketplace before 3 reference workflows ship
