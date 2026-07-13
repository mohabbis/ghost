/** Shared types for Ghost 2.0 execution review (compiled to review.js). */

export type VerificationStatus =
  | "verified"
  | "failed"
  | "skipped"
  | "not_applicable";

export interface StepVerification {
  step_id: string;
  label: string;
  expected: string;
  observed: string;
  status: VerificationStatus;
  continue_execution: boolean;
}

export interface UiTarget {
  app: string;
  role: string;
  title?: string | null;
  fingerprint?: string | null;
}

export type ActionKind =
  | { kind: "create_folder"; path: string }
  | { kind: "move_file"; from: string; to: string }
  | { kind: "rename_file"; from: string; to: string }
  | { kind: "open_application"; name: string }
  | { kind: "semantic_focus"; target: UiTarget }
  | { kind: "semantic_set_value"; target: UiTarget; value: string }
  | { kind: "semantic_verify"; target: UiTarget; expected_value?: string | null }
  | { kind: "type_text"; text: string; app?: string | null }
  | { kind: "shortcut"; combo: string }
  | { kind: "wait"; ms: number }
  | { kind: "verify_path"; path: string; should_exist: boolean }
  | { kind: "ui_replay"; events: unknown[]; step_index: number };

export interface ActionStep {
  id: string;
  label: string;
  kind: ActionKind;
  capability: Record<string, unknown>;
  decision: Record<string, unknown>;
  confidence?: number;
  reason?: string;
}

export interface ActionPlan {
  version: number;
  id: string;
  title: string;
  source: Record<string, unknown>;
  steps: ActionStep[];
  summary: {
    total_steps: number;
    filesystem_steps: number;
    ui_steps: number;
    verify_steps: number;
    denied: number;
    needs_confirmation: number;
  };
}

export interface ExecutionReceipt {
  execution_id?: string | null;
  plan_id: string;
  plan_title: string;
  started_at: string;
  finished_at: string;
  applied: number;
  skipped: number;
  failed: number;
  stopped_early: boolean;
  stop_reason?: string | null;
  undo_available: boolean;
  steps: Array<{
    id: string;
    label: string;
    outcome: string;
    verification: StepVerification;
  }>;
  audit_event_count: number;
}
