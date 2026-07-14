import Foundation
import Combine

@MainActor
final class HistoryStore: ObservableObject {
    private let core: any GhostCoreClient

    @Published private(set) var executions: [ExecutionSummary] = []
    @Published private(set) var isLoading = false
    @Published private(set) var errorMessage: String?

    init(core: any GhostCoreClient) {
        self.core = core
    }

    func refresh() {
        guard !isLoading else { return }
        isLoading = true
        errorMessage = nil
        Task { [weak self] in
            guard let self else { return }
            defer { isLoading = false }
            do {
                executions = try await core.listExecutions()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}
