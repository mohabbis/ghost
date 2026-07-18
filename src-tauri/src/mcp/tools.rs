/// Narrow MCP tool surface — read/plan/approve/execute-approved only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpToolKind {
    Status,
    ListZones,
    ScanZone,
    CreatePlan,
    GetPlan,
    ExplainPlan,
    ValidatePlan,
    RequestApproval,
    GetApprovalStatus,
    ExecuteApprovedPlan,
    GetRun,
    UndoRun,
    ListRoutines,
    PreviewRoutine,
    RequestRoutineApproval,
    ExecuteApprovedRoutine,
}

impl McpToolKind {
    pub fn all() -> &'static [McpToolKind] {
        &[
            McpToolKind::Status,
            McpToolKind::ListZones,
            McpToolKind::ScanZone,
            McpToolKind::CreatePlan,
            McpToolKind::GetPlan,
            McpToolKind::ExplainPlan,
            McpToolKind::ValidatePlan,
            McpToolKind::RequestApproval,
            McpToolKind::GetApprovalStatus,
            McpToolKind::ExecuteApprovedPlan,
            McpToolKind::GetRun,
            McpToolKind::UndoRun,
            McpToolKind::ListRoutines,
            McpToolKind::PreviewRoutine,
            McpToolKind::RequestRoutineApproval,
            McpToolKind::ExecuteApprovedRoutine,
        ]
    }

    pub fn description(self) -> &'static str {
        match self {
            McpToolKind::Status => {
                "Ghost app status, trust pipeline, and how macOS semantic/vision ops are exposed (approval-bound only)"
            }
            McpToolKind::ListZones => "List user-approved Zones (metadata only)",
            McpToolKind::ScanZone => "Scan a Zone and return file metadata (read-only)",
            McpToolKind::CreatePlan => "Create a deterministic Organizer plan (no execution)",
            McpToolKind::GetPlan => "Get the current plan for a Zone",
            McpToolKind::ExplainPlan => "Explain how Ghost plans Organizer actions",
            McpToolKind::ValidatePlan => "Re-run policy checks on a Zone plan",
            McpToolKind::RequestApproval => "Request desktop approval (opens Ghost UI)",
            McpToolKind::GetApprovalStatus => "Check approval status for a pending plan",
            McpToolKind::ExecuteApprovedPlan => "Execute a plan approved in Ghost desktop UI",
            McpToolKind::GetRun => "Get run summary and audit metadata",
            McpToolKind::UndoRun => "Undo an eligible completed run",
            McpToolKind::ListRoutines => {
                "List saved routine names only (no recorded input or typed text)"
            }
            McpToolKind::PreviewRoutine => {
                "Preview redacted compressed steps, compiled Action Plan (incl. semantic UI reliability), and policy — no execution, no AX/OCR/capture"
            }
            McpToolKind::RequestRoutineApproval => {
                "Request local Ghost review for an exact saved routine"
            }
            McpToolKind::ExecuteApprovedRoutine => {
                "Execute an unchanged routine approved in Ghost via Action Plan runtime (AX→OCR→template for Semantic* steps); returns receipt — requires plan-hash token"
            }
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            McpToolKind::Status => "ghost.status",
            McpToolKind::ListZones => "ghost.list_zones",
            McpToolKind::ScanZone => "ghost.scan_zone",
            McpToolKind::CreatePlan => "ghost.create_plan",
            McpToolKind::GetPlan => "ghost.get_plan",
            McpToolKind::ExplainPlan => "ghost.explain_plan",
            McpToolKind::ValidatePlan => "ghost.validate_plan",
            McpToolKind::RequestApproval => "ghost.request_approval",
            McpToolKind::GetApprovalStatus => "ghost.get_approval_status",
            McpToolKind::ExecuteApprovedPlan => "ghost.execute_approved_plan",
            McpToolKind::GetRun => "ghost.get_run",
            McpToolKind::UndoRun => "ghost.undo_run",
            McpToolKind::ListRoutines => "ghost.list_routines",
            McpToolKind::PreviewRoutine => "ghost.preview_routine",
            McpToolKind::RequestRoutineApproval => "ghost.request_routine_approval",
            McpToolKind::ExecuteApprovedRoutine => "ghost.execute_approved_routine",
        }
    }

    pub fn requires_approval(self) -> bool {
        matches!(
            self,
            McpToolKind::ExecuteApprovedPlan | McpToolKind::ExecuteApprovedRoutine
        )
    }

    pub fn mutates(self) -> bool {
        matches!(
            self,
            McpToolKind::ExecuteApprovedPlan
                | McpToolKind::ExecuteApprovedRoutine
                | McpToolKind::UndoRun
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_requires_approval() {
        assert!(McpToolKind::ExecuteApprovedPlan.requires_approval());
        assert!(McpToolKind::ExecuteApprovedRoutine.requires_approval());
        assert!(!McpToolKind::CreatePlan.requires_approval());
    }
}
