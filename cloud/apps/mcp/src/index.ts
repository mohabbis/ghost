/**
 * Ghost Cloud MCP server (stdio, JSON-RPC 2.0).
 *
 * Exposes allow-listed tools from `@ghost/core/agent`. Approval is deliberately
 * absent — humans approve in the Ghost web UI.
 *
 * Env:
 *   GHOST_API_URL           default http://localhost:3000
 *   GHOST_ACCESS_TOKEN      required (created in Ghost Settings)
 *
 * Cursor example (~/.cursor/mcp.json):
 * {
 *   "mcpServers": {
 *     "ghost": {
 *       "command": "pnpm",
 *       "args": ["--filter", "@ghost/mcp", "exec", "tsx", "src/index.ts"],
 *       "cwd": "<repo>/cloud",
 *       "env": {
 *         "GHOST_API_URL": "http://localhost:3000",
 *         "GHOST_ACCESS_TOKEN": "ghost_agent_…"
 *       }
 *     }
 *   }
 * }
 */

import { createInterface } from "node:readline";
import { AGENT_TOOLS } from "@ghost/core/agent";
import { GhostAgentClient } from "./client.js";

type JsonRpcId = string | number | null;
type JsonRpcRequest = {
  jsonrpc?: string;
  id?: JsonRpcId;
  method?: string;
  params?: unknown;
};

const PROTOCOL_VERSION = "2024-11-05";

function ok(id: JsonRpcId | undefined, result: unknown) {
  return JSON.stringify({ jsonrpc: "2.0", id: id ?? null, result });
}

function fail(id: JsonRpcId | undefined, code: number, message: string) {
  return JSON.stringify({
    jsonrpc: "2.0",
    id: id ?? null,
    error: { code, message },
  });
}

function requireEnv(name: string): string {
  const v = process.env[name]?.trim();
  if (!v) throw new Error(`${name} is required`);
  return v;
}

function toolList() {
  return {
    tools: AGENT_TOOLS.map((t) => ({
      name: t.name,
      description: t.description,
      inputSchema: t.inputSchema,
    })),
  };
}

async function handle(
  client: GhostAgentClient,
  msg: JsonRpcRequest,
): Promise<string | null> {
  const { id, method, params } = msg;
  if (!method) return fail(id, -32600, "missing method");

  // Notifications have no id — acknowledge silently where needed.
  const isNotification = id === undefined;

  switch (method) {
    case "initialize":
      return ok(id, {
        protocolVersion: PROTOCOL_VERSION,
        capabilities: { tools: {} },
        serverInfo: {
          name: "ghost-cloud",
          version: "0.0.0",
          description:
            "Ghost Cloud trust runtime. Agents may list/preview/start runs; humans approve sensitive steps in the Ghost UI.",
        },
      });
    case "notifications/initialized":
    case "initialized":
      return null;
    case "ping":
      return isNotification ? null : ok(id, {});
    case "tools/list":
      return ok(id, toolList());
    case "tools/call": {
      const p = (params ?? {}) as {
        name?: string;
        arguments?: Record<string, unknown>;
      };
      const name = typeof p.name === "string" ? p.name : "";
      if (!name) return fail(id, -32602, "tools/call requires name");
      try {
        const data = (await client.invoke(name, p.arguments ?? {})) as {
          result?: unknown;
        };
        const payload = data?.result ?? data;
        return ok(id, {
          content: [{ type: "text", text: JSON.stringify(payload, null, 2) }],
          structuredContent: payload,
        });
      } catch (err) {
        const message = err instanceof Error ? err.message : "tool call failed";
        return ok(id, {
          content: [{ type: "text", text: message }],
          isError: true,
        });
      }
    }
    default:
      if (isNotification) return null;
      return fail(id, -32601, `method not found: ${method}`);
  }
}

async function main() {
  const apiKey = requireEnv("GHOST_ACCESS_TOKEN");
  const baseUrl = process.env.GHOST_API_URL?.trim() || "http://localhost:3000";
  const client = new GhostAgentClient({ baseUrl, apiKey });

  const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of rl) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    let msg: JsonRpcRequest;
    try {
      msg = JSON.parse(trimmed) as JsonRpcRequest;
    } catch {
      process.stdout.write(fail(null, -32700, "parse error") + "\n");
      continue;
    }
    const out = await handle(client, msg);
    if (out) process.stdout.write(out + "\n");
  }
}

main().catch((err) => {
  console.error(err instanceof Error ? err.message : err);
  process.exit(1);
});
