import SwiftUI

struct HistoryView: View {
    @StateObject private var store: HistoryStore

    init(core: any GhostCoreClient) {
        _store = StateObject(wrappedValue: HistoryStore(core: core))
    }

    var body: some View {
        Group {
            if store.isLoading && store.executions.isEmpty {
                VStack(spacing: 12) {
                    ProgressView()
                    Text("Loading local execution history")
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let error = store.errorMessage, store.executions.isEmpty {
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.system(size: 36))
                        .foregroundStyle(.orange)
                    Text("History is unavailable")
                        .font(.title2.weight(.semibold))
                    Text(error)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 560)
                    Button("Try Again", action: store.refresh)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .padding()
            } else if store.executions.isEmpty {
                VStack(spacing: 12) {
                    Image(systemName: "clock.arrow.circlepath")
                        .font(.system(size: 38))
                        .foregroundStyle(.secondary)
                    Text("No executions yet")
                        .font(.title2.weight(.semibold))
                    Text("Completed Organizer runs will appear here with their audit and recovery status.")
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 520)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .padding()
            } else {
                List(store.executions) { execution in
                    ExecutionSummaryRow(execution: execution)
                }
                .listStyle(.inset)
            }
        }
        .navigationTitle("History")
        .toolbar {
            ToolbarItem {
                Button {
                    store.refresh()
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .disabled(store.isLoading)
            }
        }
        .task {
            store.refresh()
        }
    }
}

private struct ExecutionSummaryRow: View {
    let execution: ExecutionSummary

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: execution.finished ? "checkmark.seal" : "exclamationmark.arrow.triangle.2.circlepath")
                .foregroundStyle(execution.finished ? .green : .orange)
                .frame(width: 22)

            VStack(alignment: .leading, spacing: 4) {
                Text(execution.finished ? "Organizer execution" : "Interrupted execution")
                    .font(.body.weight(.medium))
                Text(execution.id.rawValue)
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                Text(formatEpoch(execution.createdAt))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            HStack(spacing: 12) {
                HistoryMetric(value: execution.applied, label: "Applied")
                HistoryMetric(value: execution.skipped, label: "Skipped")
                HistoryMetric(value: execution.failed, label: "Failed")
                Image(systemName: execution.sealed ? "lock.shield.fill" : "lock.open")
                    .help(execution.sealed ? "Audit record is sealed" : "Legacy or unsealed record")
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 6)
    }

    private func formatEpoch(_ raw: String) -> String {
        guard let seconds = TimeInterval(raw) else { return raw }
        return Date(timeIntervalSince1970: seconds).formatted(date: .abbreviated, time: .shortened)
    }
}

private struct HistoryMetric: View {
    let value: Int
    let label: String

    var body: some View {
        VStack(alignment: .trailing, spacing: 1) {
            Text(value.formatted())
                .font(.callout.weight(.medium).monospacedDigit())
            Text(label)
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
    }
}
