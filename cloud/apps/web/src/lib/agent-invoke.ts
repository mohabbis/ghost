import {
  getAgentTool,
  isForbiddenAgentTool,
  type AgentToolName,
} from "@ghost/core/agent";
import { prisma } from "@/lib/db";
import { enqueueRunWorkflow } from "@/lib/queue";
import type { AgentPrincipal } from "@/lib/agent-auth";

export type InvokeResult =
  | { ok: true; result: unknown }
  | { ok: false; status: number; error: string };

function asRecord(args: unknown): Record<string, unknown> {
  if (args && typeof args === "object" && !Array.isArray(args)) {
    return args as Record<string, unknown>;
  }
  return {};
}

function requireString(args: Record<string, unknown>, key: string): string | null {
  const v = args[key];
  return typeof v === "string" && v.trim() ? v.trim() : null;
}

async function listWorkflows(principal: AgentPrincipal, args: Record<string, unknown>) {
  const includeArchived = args.includeArchived === true;
  const workflows = await prisma.workflow.findMany({
    where: {
      orgId: principal.orgId,
      ...(includeArchived ? {} : { archived: false }),
    },
    orderBy: { createdAt: "desc" },
    select: {
      id: true,
      name: true,
      description: true,
      archived: true,
      createdAt: true,
      versions: {
        orderBy: { version: "desc" },
        take: 1,
        select: { version: true },
      },
    },
  });
  return {
    workflows: workflows.map((w) => ({
      id: w.id,
      name: w.name,
      description: w.description,
      archived: w.archived,
      createdAt: w.createdAt,
      latestVersion: w.versions[0]?.version ?? null,
    })),
  };
}

type HandlerFail = { ok: false; status: number; error: string };
type HandlerOk<T> = { ok: true; data: T };
type HandlerResult<T> = HandlerOk<T> | HandlerFail;

async function getWorkflow(
  principal: AgentPrincipal,
  args: Record<string, unknown>,
): Promise<HandlerResult<Record<string, unknown>>> {
  const workflowId = requireString(args, "workflowId");
  if (!workflowId) return { ok: false, error: "workflowId required", status: 400 };

  const workflow = await prisma.workflow.findFirst({
    where: { id: workflowId, orgId: principal.orgId },
    include: {
      versions: { orderBy: { version: "desc" }, take: 1 },
    },
  });
  if (!workflow) return { ok: false, error: "workflow not found", status: 404 };

  const latest = workflow.versions[0];
  return {
    ok: true,
    data: {
      id: workflow.id,
      name: workflow.name,
      description: workflow.description,
      archived: workflow.archived,
      latestVersion: latest
        ? { version: latest.version, note: latest.note, steps: latest.steps }
        : null,
    },
  };
}

async function startRun(
  principal: AgentPrincipal,
  args: Record<string, unknown>,
): Promise<HandlerResult<Record<string, unknown>>> {
  const workflowId = requireString(args, "workflowId");
  if (!workflowId) return { ok: false, error: "workflowId required", status: 400 };

  const version = await prisma.workflowVersion.findFirst({
    where: { workflowId, workflow: { orgId: principal.orgId } },
    orderBy: { version: "desc" },
  });
  if (!version) return { ok: false, error: "workflow not found", status: 404 };

  const run = await prisma.run.create({
    data: {
      orgId: principal.orgId,
      workflowVersionId: version.id,
      triggeredById: principal.userId,
      status: "QUEUED",
    },
  });

  await enqueueRunWorkflow({ runId: run.id, orgId: principal.orgId });

  return {
    ok: true,
    data: {
      runId: run.id,
      status: "QUEUED",
      note: "Sensitive steps will halt for human approval in the Ghost UI. Agents cannot approve.",
    },
  };
}

async function getRun(
  principal: AgentPrincipal,
  args: Record<string, unknown>,
): Promise<HandlerResult<Record<string, unknown>>> {
  const runId = requireString(args, "runId");
  if (!runId) return { ok: false, error: "runId required", status: 400 };

  const run = await prisma.run.findFirst({
    where: { id: runId, orgId: principal.orgId },
    include: {
      steps: { orderBy: { index: "asc" } },
      approvals: { orderBy: { stepIndex: "asc" } },
      workflowVersion: { include: { workflow: { select: { id: true, name: true } } } },
    },
  });
  if (!run) return { ok: false, error: "run not found", status: 404 };

  return {
    ok: true,
    data: {
      id: run.id,
      status: run.status,
      error: run.error,
      workflowId: run.workflowVersion.workflow.id,
      workflowName: run.workflowVersion.workflow.name,
      startedAt: run.startedAt,
      endedAt: run.endedAt,
      steps: run.steps.map((s) => ({
        index: s.index,
        type: s.type,
        label: s.label,
        status: s.status,
        verification: s.verification,
        error: s.error,
        screenshotUrl: s.screenshotKey ? `/api/artifacts/${s.screenshotKey}` : null,
      })),
      approvals: run.approvals.map((a) => ({
        stepIndex: a.stepIndex,
        status: a.status,
        reason: a.reason,
      })),
      approvalHint:
        run.status === "AWAITING_APPROVAL"
          ? "Open this run in the Ghost web UI to approve or reject. Agents cannot approve."
          : null,
    },
  };
}

async function listPendingApprovals(principal: AgentPrincipal) {
  const runs = await prisma.run.findMany({
    where: { orgId: principal.orgId, status: "AWAITING_APPROVAL" },
    orderBy: { startedAt: "desc" },
    include: {
      approvals: { where: { status: "PENDING" }, orderBy: { stepIndex: "asc" } },
      workflowVersion: { include: { workflow: { select: { id: true, name: true } } } },
    },
  });

  return {
    pending: runs.map((r) => ({
      runId: r.id,
      workflowId: r.workflowVersion.workflow.id,
      workflowName: r.workflowVersion.workflow.name,
      status: r.status,
      startedAt: r.startedAt,
      approvals: r.approvals.map((a) => ({
        stepIndex: a.stepIndex,
        reason: a.reason,
        status: a.status,
      })),
    })),
    note: "Approve in the Ghost UI (/runs/[id]). Agents cannot approve or reject.",
  };
}

/**
 * Dispatch an agent tool. Forbidden names (approve/reject) always 403.
 */
export async function invokeAgentTool(
  principal: AgentPrincipal,
  name: string,
  rawArgs: unknown,
): Promise<InvokeResult> {
  if (isForbiddenAgentTool(name)) {
    return {
      ok: false,
      status: 403,
      error:
        "Agents cannot approve or reject. Open the run in the Ghost UI; a human must decide.",
    };
  }

  const tool = getAgentTool(name);
  if (!tool) {
    return { ok: false, status: 404, error: `unknown tool: ${name}` };
  }

  const args = asRecord(rawArgs);

  try {
    switch (tool.name as AgentToolName) {
      case "list_workflows":
        return { ok: true, result: await listWorkflows(principal, args) };
      case "get_workflow": {
        const out = await getWorkflow(principal, args);
        if (!out.ok) return out;
        return { ok: true, result: out.data };
      }
      case "start_run": {
        const out = await startRun(principal, args);
        if (!out.ok) return out;
        return { ok: true, result: out.data };
      }
      case "get_run": {
        const out = await getRun(principal, args);
        if (!out.ok) return out;
        return { ok: true, result: out.data };
      }
      case "list_pending_approvals":
        return { ok: true, result: await listPendingApprovals(principal) };
      default:
        return { ok: false, status: 404, error: `unknown tool: ${name}` };
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : "invoke failed";
    return { ok: false, status: 500, error: message };
  }
}
