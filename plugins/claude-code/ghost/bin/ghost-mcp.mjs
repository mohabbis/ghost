#!/usr/bin/env node
import { createInterface } from "node:readline";

const tools = [
  [
    "list_workflows",
    "List available Ghost workflows.",
    { type: "object", properties: {}, additionalProperties: false },
  ],
  [
    "get_workflow",
    "Preview a Ghost workflow and its current steps.",
    {
      type: "object",
      properties: { workflowId: { type: "string" } },
      required: ["workflowId"],
      additionalProperties: false,
    },
  ],
  [
    "start_run",
    "Start a Ghost workflow run. Sensitive steps still require approval in Ghost.",
    {
      type: "object",
      properties: { workflowId: { type: "string" } },
      required: ["workflowId"],
      additionalProperties: false,
    },
  ],
  [
    "get_run",
    "Read a Ghost run, its step results, and approval state.",
    {
      type: "object",
      properties: { runId: { type: "string" } },
      required: ["runId"],
      additionalProperties: false,
    },
  ],
  [
    "list_pending_approvals",
    "List approvals waiting for a human in Ghost.",
    { type: "object", properties: {}, additionalProperties: false },
  ],
].map(([name, description, inputSchema]) => ({
  name,
  description,
  inputSchema,
}));

const baseUrl = (process.env.GHOST_API_URL || "http://localhost:3000").replace(
  /\/$/,
  "",
);
const token = process.env.GHOST_ACCESS_TOKEN?.trim();
if (!token) {
  console.error(
    "GHOST_ACCESS_TOKEN is required. Create one in Ghost Settings.",
  );
  process.exit(1);
}

const ok = (id, result) => ({ jsonrpc: "2.0", id: id ?? null, result });
const fail = (id, code, message) => ({
  jsonrpc: "2.0",
  id: id ?? null,
  error: { code, message },
});

async function invoke(name, args) {
  const response = await fetch(`${baseUrl}/api/agent/invoke`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ name, arguments: args }),
  });
  const data = await response.json().catch(() => ({}));
  if (!response.ok)
    throw new Error(data.error || `Ghost returned HTTP ${response.status}`);
  return data.result ?? data;
}

async function handle(message) {
  const { id, method, params } = message;
  if (method === "initialize")
    return ok(id, {
      protocolVersion: "2024-11-05",
      capabilities: { tools: {} },
      serverInfo: { name: "ghost", version: "0.1.0" },
    });
  if (method === "notifications/initialized" || method === "initialized")
    return null;
  if (method === "ping") return id === undefined ? null : ok(id, {});
  if (method === "tools/list") return ok(id, { tools });
  if (method === "tools/call") {
    try {
      const result = await invoke(params?.name, params?.arguments || {});
      return ok(id, {
        content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        structuredContent: result,
      });
    } catch (error) {
      return ok(id, {
        content: [
          {
            type: "text",
            text: error instanceof Error ? error.message : "Ghost call failed",
          },
        ],
        isError: true,
      });
    }
  }
  return id === undefined
    ? null
    : fail(id, -32601, `method not found: ${method}`);
}

for await (const line of createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
})) {
  if (!line.trim()) continue;
  let response;
  try {
    response = await handle(JSON.parse(line));
  } catch {
    response = fail(null, -32700, "parse error");
  }
  if (response) process.stdout.write(`${JSON.stringify(response)}\n`);
}
