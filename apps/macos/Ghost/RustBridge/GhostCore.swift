import Foundation

protocol GhostCoreClient: AnyObject, Sendable {
    func handshake() async throws -> CoreHandshake
    func authStatus() async throws -> AuthStatus
    func unlock(password: String) async throws -> AuthStatus
    func lock() async throws -> AuthStatus
    func scan(folder: URL) async throws -> ScanResult
    func createPlan(scanID: ScanID) async throws -> ExecutionPlan
    func approve(planID: PlanID) async throws -> ApprovalToken
    func execute(
        planID: PlanID,
        approvalToken: ApprovalToken
    ) async -> AsyncThrowingStream<ExecutionEvent, Error>
    func receipt(executionID: ExecutionID) async throws -> ExecutionReceipt
    func undo(executionID: ExecutionID) async -> AsyncThrowingStream<UndoEvent, Error>
    func listExecutions() async throws -> [ExecutionSummary]
}

enum GhostCoreError: LocalizedError, Sendable {
    case bridgeUnavailable(String)
    case invalidResponse(String)
    case remote(code: String, message: String, recoverable: Bool)
    case processTerminated(String)

    var errorDescription: String? {
        switch self {
        case .bridgeUnavailable(let message):
            return "Ghost's Rust bridge is unavailable: \(message)"
        case .invalidResponse(let message):
            return "Ghost received an invalid bridge response: \(message)"
        case .remote(_, let message, _):
            return message
        case .processTerminated(let message):
            return "Ghost's Rust bridge stopped: \(message)"
        }
    }

    var isRecoverable: Bool {
        switch self {
        case .remote(_, _, let recoverable):
            return recoverable
        case .invalidResponse:
            return true
        case .bridgeUnavailable, .processTerminated:
            return false
        }
    }
}

actor UnavailableGhostCoreClient: GhostCoreClient {
    private let reason: String

    init(reason: String) {
        self.reason = reason
    }

    private func failure() -> GhostCoreError {
        .bridgeUnavailable(reason)
    }

    func handshake() async throws -> CoreHandshake { throw failure() }
    func authStatus() async throws -> AuthStatus { throw failure() }
    func unlock(password: String) async throws -> AuthStatus { throw failure() }
    func lock() async throws -> AuthStatus { throw failure() }
    func scan(folder: URL) async throws -> ScanResult { throw failure() }
    func createPlan(scanID: ScanID) async throws -> ExecutionPlan { throw failure() }
    func approve(planID: PlanID) async throws -> ApprovalToken { throw failure() }

    func execute(
        planID: PlanID,
        approvalToken: ApprovalToken
    ) async -> AsyncThrowingStream<ExecutionEvent, Error> {
        let error = failure()
        return AsyncThrowingStream { continuation in
            continuation.finish(throwing: error)
        }
    }

    func receipt(executionID: ExecutionID) async throws -> ExecutionReceipt { throw failure() }

    func undo(executionID: ExecutionID) async -> AsyncThrowingStream<UndoEvent, Error> {
        let error = failure()
        return AsyncThrowingStream { continuation in
            continuation.finish(throwing: error)
        }
    }

    func listExecutions() async throws -> [ExecutionSummary] { throw failure() }
}
