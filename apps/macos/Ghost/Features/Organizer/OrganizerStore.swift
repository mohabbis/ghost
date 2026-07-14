import Foundation
import Combine

@MainActor
enum OrganizerPhase: Equatable {
    case idle
    case readyToScan
    case scanning
    case scanned
    case planning
    case reviewing
    case approving
    case approved
    case executing
    case completed
    case undoing
    case undone
    case failed
}

struct ExecutionProgressState: Equatable {
    let executionID: ExecutionID?
    let completed: Int
    let total: Int
    let applied: Int
    let skipped: Int
    let failed: Int
    let currentStep: String?

    static let empty = ExecutionProgressState(
        executionID: nil,
        completed: 0,
        total: 0,
        applied: 0,
        skipped: 0,
        failed: 0,
        currentStep: nil
    )

    var fraction: Double {
        guard total > 0 else { return 0 }
        return min(1, Double(completed) / Double(total))
    }
}

@MainActor
final class OrganizerStore: ObservableObject {
    private let core: any GhostCoreClient
    private let filePanel: FilePanelService
    private let bookmarkStore: SecurityScopedBookmarkStore

    private var folderGrant: FolderAccessGrant?
    private var approvalToken: ApprovalToken?

    @Published private(set) var phase: OrganizerPhase = .idle
    @Published private(set) var selectedFolder: URL?
    @Published private(set) var scanResult: ScanResult?
    @Published private(set) var plan: ExecutionPlan?
    @Published private(set) var receipt: ExecutionReceipt?
    @Published private(set) var undoReport: UndoReport?
    @Published private(set) var progress: ExecutionProgressState = .empty
    @Published private(set) var errorMessage: String?

    init(
        core: any GhostCoreClient,
        filePanel: FilePanelService,
        bookmarkStore: SecurityScopedBookmarkStore
    ) {
        self.core = core
        self.filePanel = filePanel
        self.bookmarkStore = bookmarkStore

        do {
            if let grant = try bookmarkStore.restoreLastGrant() {
                folderGrant = grant
                selectedFolder = grant.url
                phase = .readyToScan
            }
        } catch {
            bookmarkStore.clear()
        }
    }

    var canScan: Bool {
        selectedFolder != nil && !isBusy
    }

    var canCreatePlan: Bool {
        scanResult != nil && phase == .scanned
    }

    var canApprove: Bool {
        plan != nil && phase == .reviewing
    }

    var canExecute: Bool {
        approvalToken != nil && phase == .approved
    }

    var canUndo: Bool {
        receipt?.undoAvailable == true && receipt?.executionID != nil && phase == .completed
    }

    var isBusy: Bool {
        [.scanning, .planning, .approving, .executing, .undoing].contains(phase)
    }

    func chooseFolder() {
        guard !isBusy, let url = filePanel.chooseFolder() else { return }
        do {
            folderGrant = try bookmarkStore.grant(for: url)
            selectedFolder = folderGrant?.url ?? url
        } catch {
            folderGrant = FolderAccessGrant(url: url)
            selectedFolder = url
        }
        clearWorkflow(keepFolder: true)
        phase = .readyToScan
    }

    func scan() {
        guard let folder = selectedFolder, canScan else { return }
        phase = .scanning
        errorMessage = nil
        scanResult = nil
        plan = nil
        receipt = nil
        undoReport = nil
        approvalToken = nil
        progress = .empty

        Task { [weak self] in
            guard let self else { return }
            do {
                let result = try await core.scan(folder: folder)
                scanResult = result
                phase = .scanned
            } catch {
                fail(error)
            }
        }
    }

    func createPlan() {
        guard let scanID = scanResult?.scanID, canCreatePlan else { return }
        phase = .planning
        errorMessage = nil

        Task { [weak self] in
            guard let self else { return }
            do {
                plan = try await core.createPlan(scanID: scanID)
                phase = .reviewing
            } catch {
                fail(error)
            }
        }
    }

    func approvePlan() {
        guard let planID = plan?.planID, canApprove else { return }
        phase = .approving
        errorMessage = nil

        Task { [weak self] in
            guard let self else { return }
            do {
                approvalToken = try await core.approve(planID: planID)
                phase = .approved
            } catch {
                fail(error)
            }
        }
    }

    func executeApprovedPlan() {
        guard let plan, let approvalToken, canExecute else { return }
        phase = .executing
        errorMessage = nil
        progress = ExecutionProgressState(
            executionID: nil,
            completed: 0,
            total: plan.steps.count,
            applied: 0,
            skipped: 0,
            failed: 0,
            currentStep: nil
        )

        Task { [weak self] in
            guard let self else { return }
            let stream = await core.execute(
                planID: plan.planID,
                approvalToken: approvalToken
            )
            do {
                for try await event in stream {
                    switch event {
                    case .started(let executionID, let totalSteps):
                        progress = ExecutionProgressState(
                            executionID: executionID,
                            completed: 0,
                            total: totalSteps,
                            applied: 0,
                            skipped: 0,
                            failed: 0,
                            currentStep: nil
                        )
                    case .progress(
                        let executionID,
                        let completed,
                        let total,
                        let applied,
                        let skipped,
                        let failed,
                        let currentStep
                    ):
                        progress = ExecutionProgressState(
                            executionID: executionID,
                            completed: completed,
                            total: total,
                            applied: applied,
                            skipped: skipped,
                            failed: failed,
                            currentStep: currentStep
                        )
                    case .completed(let finalReceipt):
                        receipt = finalReceipt
                        self.approvalToken = nil
                        phase = .completed
                    }
                }
            } catch {
                self.approvalToken = nil
                fail(error)
            }
        }
    }

    func undoLastExecution() {
        guard let executionID = receipt?.executionID, canUndo else { return }
        phase = .undoing
        errorMessage = nil

        Task { [weak self] in
            guard let self else { return }
            let stream = await core.undo(executionID: executionID)
            do {
                for try await event in stream {
                    switch event {
                    case .started:
                        break
                    case .completed(let report):
                        undoReport = report
                        phase = .undone
                    }
                }
            } catch {
                fail(error)
            }
        }
    }

    func startOver() {
        clearWorkflow(keepFolder: true)
        phase = selectedFolder == nil ? .idle : .readyToScan
    }

    func clearFolder() {
        bookmarkStore.clear()
        folderGrant = nil
        selectedFolder = nil
        clearWorkflow(keepFolder: false)
        phase = .idle
    }

    private func clearWorkflow(keepFolder: Bool) {
        if !keepFolder {
            selectedFolder = nil
        }
        scanResult = nil
        plan = nil
        receipt = nil
        undoReport = nil
        approvalToken = nil
        progress = .empty
        errorMessage = nil
    }

    private func fail(_ error: Error) {
        errorMessage = error.localizedDescription
        phase = .failed
    }
}
