import SwiftUI

struct PlanReviewView: View {
    let plan: ExecutionPlan
    let approve: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Review the exact plan")
                        .font(.title2.weight(.semibold))
                    Text("Nothing has changed yet. Denied actions remain visible and will not run.")
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Approve Plan", action: approve)
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.return, modifiers: [.command])
            }

            PlanSummaryView(summary: plan.summary)

            List(plan.steps) { step in
                PlanStepRow(step: step)
            }
            .listStyle(.inset)
            .frame(minHeight: 300)

            GroupBox {
                HStack(alignment: .top, spacing: 10) {
                    Image(systemName: "lock.shield")
                        .foregroundStyle(.secondary)
                    Text("Approval is bound to this exact plan hash and expires after five minutes. Rust replans before execution and refuses to run if the effective plan changed.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }
}

private struct PlanSummaryView: View {
    let summary: PlanSummary

    var body: some View {
        HStack(spacing: 10) {
            SummaryPill(value: summary.filesScanned, label: "Scanned")
            SummaryPill(value: summary.totalSteps, label: "Steps")
            SummaryPill(value: summary.needsConfirmation, label: "Need approval")
            SummaryPill(value: summary.conflicts, label: "Conflicts")
            SummaryPill(value: summary.denied, label: "Denied")
            Spacer()
        }
    }
}

private struct SummaryPill: View {
    let value: Int
    let label: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(value.formatted())
                .font(.headline.monospacedDigit())
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 9))
    }
}

private struct PlanStepRow: View {
    let step: PlanStep

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: step.kind.systemImage)
                .frame(width: 20)
                .foregroundStyle(step.decision.tint)

            VStack(alignment: .leading, spacing: 4) {
                Text(step.label)
                    .font(.body.weight(.medium))
                    .lineLimit(2)
                if !step.reason.isEmpty {
                    Text(step.reason)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }
                if let conflict = step.conflict {
                    Label(conflict, systemImage: "exclamationmark.triangle")
                        .font(.caption)
                        .foregroundStyle(.orange)
                }
            }

            Spacer(minLength: 12)

            VStack(alignment: .trailing, spacing: 5) {
                Text(step.decision.label)
                    .font(.caption.weight(.semibold))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 3)
                    .background(step.decision.tint.opacity(0.12), in: Capsule())
                    .foregroundStyle(step.decision.tint)
                Text(step.confidence, format: .percent.precision(.fractionLength(0)))
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 4)
    }
}

private extension PlanStepKind {
    var systemImage: String {
        switch self {
        case .createFolder: return "folder.badge.plus"
        case .moveFile: return "arrow.right.doc.on.clipboard"
        case .renameFile: return "pencil"
        case .verifyPath: return "checkmark.shield"
        case .unsupported: return "questionmark.square.dashed"
        }
    }
}

private extension PolicyDecisionKind {
    var label: String {
        switch self {
        case .allow: return "Allowed"
        case .requireConfirmation: return "Confirm"
        case .deny: return "Denied"
        }
    }

    var tint: Color {
        switch self {
        case .allow: return .green
        case .requireConfirmation: return .orange
        case .deny: return .red
        }
    }
}
