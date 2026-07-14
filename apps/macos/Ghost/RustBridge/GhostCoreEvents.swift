import Foundation

enum ExecutionEvent: Sendable {
    case started(executionID: ExecutionID, totalSteps: Int)
    case progress(
        executionID: ExecutionID,
        completed: Int,
        total: Int,
        applied: Int,
        skipped: Int,
        failed: Int,
        currentStep: String?
    )
    case completed(ExecutionReceipt)
}

enum UndoEvent: Sendable {
    case started(executionID: ExecutionID)
    case completed(UndoReport)
}
