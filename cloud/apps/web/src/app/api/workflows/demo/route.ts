import { NextResponse } from "next/server";
import { Prisma } from "@ghost/core/db";
import { canPublishWorkflow } from "@ghost/core/roles";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { loadActor } from "@/lib/members";
import {
  demoWorkflowSteps,
  DEMO_WORKFLOW_NAME,
  DEMO_WORKFLOW_DESCRIPTION,
} from "@/lib/demo-workflow";

/** Create the seed demo workflow (with its first version) for the current org. */
export async function POST() {
  const session = await auth();
  if (!session?.user?.orgId || !session.user.id) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const actor = await loadActor(session.user.orgId, session.user.id);
  if (!actor || !canPublishWorkflow(actor.role)) {
    return NextResponse.json({ error: "forbidden" }, { status: 403 });
  }

  const workflow = await prisma.workflow.create({
    data: {
      orgId: session.user.orgId,
      name: DEMO_WORKFLOW_NAME,
      description: DEMO_WORKFLOW_DESCRIPTION,
      createdById: session.user.id,
      versions: {
        create: {
          version: 1,
          note: "seed",
          steps: demoWorkflowSteps() as unknown as Prisma.InputJsonValue,
        },
      },
    },
  });

  return NextResponse.json({ workflowId: workflow.id });
}
