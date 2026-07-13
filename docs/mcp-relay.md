# MCP Cloud Relay

Experimental wide-area routing for Ghost MCP when inbound firewall rules block direct HTTP.

```text
Remote MCP client
        |
        |  HTTPS POST /v1/mcp
        v
User-hosted relay (reference: mcp_relay_server)
        |
        |  outbound poll from desktop
        v
Ghost desktop (mcp_start_relay)
        |
        v
Same JSON-RPC handlers as stdio / local HTTP
```

The relay **routes JSON-RPC only**. It does not execute plans, read file contents, or bypass desktop approval.

## Product rules

- Opt-in only — user configures relay URL, device ID, and token in Settings.
- Desktop connects **outbound** (HTTPS) so home NAT is not a blocker.
- Relay URL must use `https://`.
- Pairing and bearer rules on the local HTTP server still apply to routed MCP bodies.
- Ghost does not operate a production relay service in this repo — ship your own or use the reference server for development.

## Reference relay server

```bash
GHOST_RELAY_SECRET=your-long-secret \
  cargo run --bin mcp_relay_server --features experimental -- 8790
```

Listens on `http://0.0.0.0:8790` by default. For in-process TLS on the relay itself:

```bash
openssl req -x509 -newkey rsa:4096 -keyout relay.key -out relay.crt \
  -days 365 -nodes -subj "/CN=localhost"

GHOST_RELAY_TLS_CERT=/absolute/path/relay.crt \
GHOST_RELAY_TLS_KEY=/absolute/path/relay.key \
GHOST_RELAY_SECRET=your-long-secret \
  cargo run --bin mcp_relay_server --features experimental -- 8790
```

Listens on `https://0.0.0.0:8790` when both TLS env vars are set. You can still terminate TLS at nginx/Caddy instead.

## Protocol (`/v1`)

All requests require `Authorization: Bearer <GHOST_RELAY_SECRET>`.

### Desktop registration

```http
POST /v1/device/register
Content-Type: application/json

{"device_id":"my-laptop"}
```

### Desktop long-poll

```http
GET /v1/device/poll?device_id=my-laptop
```

Returns:

```json
{"messages":[{"id":"uuid","body":"{\"jsonrpc\":\"2.0\",...}"}]}
```

Empty array when no work within ~55s.

### Desktop response

```http
POST /v1/device/respond
Content-Type: application/json

{"id":"uuid","body":"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{...}}"}
```

### Remote MCP client

```http
POST /v1/mcp
Content-Type: application/json

{"device_id":"my-laptop","body":"{\"jsonrpc\":\"2.0\",\"method\":\"initialize\",...}"}
```

Returns:

```json
{"body":"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{...}}"}
```

## Ghost commands (`--features experimental`)

| Command | Risk | Notes |
|---|---|---|
| `mcp_relay_status` | safe-read | Connected flag + saved URL/device hints |
| `mcp_start_relay` | external-mutate | Opens outbound HTTPS poll loop |
| `mcp_stop_relay` | local-mutate | Stops relay client |

## TLS

- Relay **client** requires HTTPS URL.
- Reference server supports optional in-process TLS via `GHOST_RELAY_TLS_CERT` + `GHOST_RELAY_TLS_KEY`, or plain HTTP when unset. A reverse proxy remains a valid production option.
- For LAN-only TLS without a relay, use in-process TLS on the MCP HTTP server (`tls_cert_path` / `tls_key_path` on `mcp_start_http_server`).

## Self-signed cert (local HTTP TLS)

```bash
openssl req -x509 -newkey rsa:4096 -keyout ghost-mcp.key -out ghost-mcp.crt \
  -days 365 -nodes -subj "/CN=localhost"
```

Pass absolute paths to Settings → MCP access → TLS cert/key when starting the HTTP server.
