import SwiftUI

struct ExecutionReceiptView: View {
    let receipt: ExecutionReceipt
    let undo: () -> Void
    let startOver: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Label("Execution complete", systemImage: receipt.failed == 0 ? "checkmark.seal.fill" : "exclamationmark.triangle.fill")
                        .font(.title2.weight(.semibold))
                    Text(receipt.planTitle)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                if receipt.undoAvailable {
                    Button("Undo", action: undo)
                        .keyboardShortcut("z", modifiers: [.command])
                }
                Button("Start Over", action: startOver)
                    .buttonStyle(.borderedProminent)
            }

            HStack(spacing: 12) {
                ReceiptMetric(value: receipt.applied, label: "Applied")
                ReceiptMetric(value: receipt.skipped, label: "Skipped")
                ReceiptMetric(value: receipt.failed, label: "Failed")
                ReceiptMetric(value: receipt.auditEventCount, label: "Audit events")
                Spacer()
            }

            List(receipt.steps) { step in
                HStack(alignment: .top, spacing: 10) {
                    Image(systemName: step.verification.status.systemImage)
                        .foregroundStyle(step.verification.status.tint)
                    VStack(alignment: .leading, spacing: 3) {
                        Text(step.label)
                            .font(.body.weight(.medium))
                        Text(step.verification.observed)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Text(step.outcome.replacingOccurrences(of: "_", with: " ").capitalized)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 3)
            }
            .listStyle(.inset)
            .frame(minHeight: 260)

            GroupBox("Receipt integrity") {
                VStack(alignment: .leading, spacing: 5) {
                    LabeledContent("Execution ID", value: receipt.executionID?.rawValue ?? "Unavailable")
                    LabeledContent("Plan ID", value: receipt.planID.rawValue)
                    LabeledContent("Undo", value: receipt.undoAvailable ? "Available" : "Not available")
                    if let reason = receipt.stopReason {
                        LabeledContent("Stopped", value: reason)
                    }
                }
                .textSelection(.enabled)
            }
        }
    }
}

private struct ReceiptMetric: View {
    let value: Int
    let label: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(value.formatted())
                .font(.title3.weight(.semibold).monospacedDigit())
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(10)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 9))
    }
}

private extension VerificationStatus {
    var systemImage: String {
        switch self {
        case .verified: return "checkmark.circle.fill"
        case .failed: return "xmark.octagon.fill"
        case .skipped: return "arrow.right.circle"
        case .notApplicable: return "minus.circle"
        }
    }

    var tint: Color {
        switch self {
        case .verified: return .green
        case .failed: return .red
        case .skipped: return .orange
        case .notApplicable: return .secondary
        }
    }
}
