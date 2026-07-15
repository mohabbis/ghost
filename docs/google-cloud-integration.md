# Google Cloud Storage Integration

Experimental outbound audit export to a user-chosen GCS bucket.

## Flow

```text
Sign in with Google (identity)
  -> Connect Google Cloud (separate grant)
  -> List buckets (project ID required)
  -> Bind export bucket (optional but recommended — scopes grant to one bucket)
  -> Preview export (read-only)
  -> User approves push in Settings
  -> Push JSON objects to gs://bucket/ghost-export/…
```

## Commands (`--features experimental`)

| Command | Risk | Notes |
|---|---|---|
| `google_grant_status` | safe-read | Local grant metadata |
| `google_request_grant` | external-mutate | OAuth consent for Storage scope |
| `google_revoke_grant` | local-mutate | Revokes local grant only |
| `google_list_buckets` | safe-read | Requires `project_id` argument |
| `google_export_preview` | safe-read | Same row shapes as Power BI/Fabric |
| `google_bind_export_bucket` | local-mutate | Sets `ResourceScope::Destination` on the active grant |
| `google_push_audit_export` | external-mutate | Re-derives payload server-side; PII-masked; push denied when grant is bound to a different bucket |

## Scopes

`https://www.googleapis.com/auth/devstorage.read_write` (incremental consent on top of Google sign-in).

## Not in v1

- Inbound Pub/Sub or GCS notification triggers
- Automatic bucket creation
- Cross-project IAM management
