/**
 * Ghost agent tool catalog.
 *
 * Positioning: AI agents (Claude, Cursor, Codex, …) may propose and inspect.
 * Ghost is the trust runtime — humans approve sensitive work in the Ghost UI.
 * Agents must never approve, reject, or bypass gates.
 *
 *   Agent (propose) → Ghost (approve · execute · verify · audit)
 */

export type AgentRiskClass =
  | "safe-read"
  | "sensitive-read"
  | "local-mutate"
  | "external-mutate"
  | "os-control"
  | "experimental";

export type AgentToolName =
  | "list_workflows"
  | "get_workflow"
  | "start_run"
  | "get_run"
  | "list_pending_approvals";

export type AgentToolDef = {
  name: AgentToolName;
  description: string;
  riskClass: AgentRiskClass;
  /** Zod-ish JSON Schema for MCP / OpenAPI-style clients. */
  inputSchema: {
    type: "object";
    properties: Record<string, unknown>;
    required?: string[];
    additionalProperties?: boolean;
  };
};

/**
 * Tools an agent must never be offered. Kept explicit so tests and docs
 * can assert the deny list even if a future author adds a similar name.
 */
export const FORBIDDEN_AGENT_TOOLS = [
  "approve_run",
  "reject_run",
  "resolve_approval",
  "approve_approval",
  "set_approval",
] as const;

export type ForbiddenAgentTool = (typeof FORBIDDEN_AGENT_TOOLS)[number];

export const AGENT_TOOLS: readonly AgentToolDef[] = [
  {
    name: "list_workflows",
    description:
      "List workflows in the current organization (id, name, description, archived). Safe metadata only.",
    riskClass: "safe-read",
    inputSchema: {
      type: "object",
      properties: {
        includeArchived: {
          type: "boolean",
          description: "When true, include archived workflows. Default false.",
        },
      },
      additionalProperties: false,
    },
  },
  {
    name: "get_workflow",
    description:
      "Preview a workflow and its latest version steps (typed plan). Does not execute anything.",
    riskClass: "safe-read",
    inputSchema: {
      type: "object",
      properties: {
        workflowId: { type: "string", description: "Workflow id" },
      },
      required: ["workflowId"],
      additionalProperties: false,
    },
  },
  {
    name: "start_run",
    description:
      "Enqueue a run of a workflow's latest version. Sensitive steps still halt for human approval in the Ghost UI — this tool cannot approve them.",
    riskClass: "local-mutate",
    inputSchema: {
      type: "object",
      properties: {
        workflowId: { type: "string", description: "Workflow id to run" },
      },
      required: ["workflowId"],
      additionalProperties: false,
    },
  },
  {
    name: "get_run",
    description:
      "Get run status, per-step outcomes, and approval state. Use this to see when a run is awaiting human approval.",
    riskClass: "safe-read",
    inputSchema: {
      type: "object",
      properties: {
        runId: { type: "string", description: "Run id" },
      },
      required: ["runId"],
      additionalProperties: false,
    },
  },
  {
    name: "list_pending_approvals",
    description:
      "List runs awaiting human approval in this org. Read-only — tell the user to approve in the Ghost UI. Agents cannot approve.",
    riskClass: "safe-read",
    inputSchema: {
      type: "object",
      properties: {},
      additionalProperties: false,
    },
  },
] as const;

const BY_NAME = new Map(AGENT_TOOLS.map((t) => [t.name, t]));

export function getAgentTool(name: string): AgentToolDef | undefined {
  return BY_NAME.get(name as AgentToolName);
}

export function isForbiddenAgentTool(name: string): boolean {
  return (FORBIDDEN_AGENT_TOOLS as readonly string[]).includes(name);
}

export function agentToolsCatalog() {
  return {
    product: "ghost-cloud",
    contract: "Agent proposes; Ghost executes only after human approval on sensitive steps.",
    forbidden: [...FORBIDDEN_AGENT_TOOLS],
    tools: AGENT_TOOLS.map((t) => ({
      name: t.name,
      description: t.description,
      riskClass: t.riskClass,
      inputSchema: t.inputSchema,
    })),
  };
}
