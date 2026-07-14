import SwiftUI

struct OrganizerView: View {
    @ObservedObject private var environment: AppEnvironment
    @ObservedObject private var store: OrganizerStore
    @State private var vaultPassword = ""

    init(environment: AppEnvironment) {
        _environment = ObservedObject(wrappedValue: environment)
        _store = ObservedObject(wrappedValue: environment.organizer)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                header
                if environment.authStatus.configured && !environment.authStatus.unlocked {
                    VaultUnlockBanner(
                        password: $vaultPassword,
                        isWorking: environment.isUpdatingVault,
                        errorMessage: environment.vaultError,
                        unlock: unlockVault
                    )
                }
                Divider()
                content
            }
            .padding(24)
            .frame(maxWidth: 1_000, alignment: .leading)
        }
        .navigationTitle("Organizer")
        .toolbar {
            ToolbarItemGroup {
                Button {
                    store.chooseFolder()
                } label: {
                    Label("Choose Folder", systemImage: "folder")
                }
                .disabled(store.isBusy)

                if store.canScan {
                    Button {
                        store.scan()
                    } label: {
                        Label("Scan", systemImage: "sparkle.magnifyingglass")
                    }
                    .keyboardShortcut("r", modifiers: [.command])
                }
            }
        }
    }

    private var header: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 5) {
                Text("Safe file organization")
                    .font(.largeTitle.weight(.semibold))
                Text("Scan metadata, review the exact plan, approve once, execute in Rust, and undo when needed.")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            if let folder = store.selectedFolder {
                VStack(alignment: .trailing, spacing: 3) {
                    Text("Selected folder")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text(folder.path)
                        .font(.callout.monospaced())
                        .lineLimit(2)
                        .textSelection(.enabled)
                }
                .frame(maxWidth: 360, alignment: .trailing)
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        switch store.phase {
        case .idle:
            emptyState(
                title: "Choose a folder",
                message: "Ghost will inspect file metadata only. It does not open file contents during Organizer scanning.",
                systemImage: "folder.badge.plus",
                actionTitle: "Choose Folder",
                action: store.chooseFolder
            )

        case .readyToScan:
            emptyState(
                title: "Ready to scan",
                message: "Scanning is read-only. No folders are created and no files are moved.",
                systemImage: "sparkle.magnifyingglass",
                actionTitle: "Scan Folder",
                action: store.scan
            )

        case .scanning:
            busyState(title: "Scanning metadata", message: "Reading names, sizes, extensions, and timestamps. File contents stay untouched.")

        case .scanned:
            if let result = store.scanResult {
                ScanResultView(result: result, createPlan: store.createPlan, canCreatePlan: environment.authStatus.unlocked)
            }

        case .planning:
            busyState(title: "Building a policy-checked plan", message: "Rust is classifying files, checking conflicts, and evaluating every action against the selected boundary.")

        case .reviewing:
            if let plan = store.plan {
                PlanReviewView(plan: plan, approve: store.approvePlan)
            }

        case .approving:
            busyState(title: "Issuing approval token", message: "The token is signed, single-use, short-lived, and bound to the exact plan hash.")

        case .approved:
            ApprovedPlanView(plan: store.plan, execute: store.executeApprovedPlan, startOver: store.startOver)

        case .executing:
            ExecutionProgressView(progress: store.progress)

        case .completed:
            if let receipt = store.receipt {
                ExecutionReceiptView(
                    receipt: receipt,
                    undo: store.undoLastExecution,
                    startOver: store.startOver
                )
            }

        case .undoing:
            busyState(title: "Undoing execution", message: "Rust is replaying the stored undo journal in reverse without overwriting occupied paths.")

        case .undone:
            UndoCompletedView(report: store.undoReport, startOver: store.startOver)

        case .failed:
            FailureView(
                message: store.errorMessage ?? "The operation failed.",
                retry: store.startOver
            )
        }
    }

    private func unlockVault() {
        let candidate = vaultPassword
        Task {
            if await environment.unlockVault(password: candidate) {
                vaultPassword = ""
            }
        }
    }

    private func emptyState(
        title: String,
        message: String,
        systemImage: String,
        actionTitle: String,
        action: @escaping () -> Void
    ) -> some View {
        VStack(spacing: 14) {
            Image(systemName: systemImage)
                .font(.system(size: 42))
                .foregroundStyle(.secondary)
            Text(title)
                .font(.title2.weight(.semibold))
            Text(message)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
                .frame(maxWidth: 540)
            Button(actionTitle, action: action)
                .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, minHeight: 360)
    }

    private func busyState(title: String, message: String) -> some View {
        VStack(spacing: 14) {
            ProgressView()
                .controlSize(.large)
            Text(title)
                .font(.title2.weight(.semibold))
            Text(message)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 560)
        }
        .frame(maxWidth: .infinity, minHeight: 360)
    }
}

private struct VaultUnlockBanner: View {
    @Binding var password: String
    let isWorking: Bool
    let errorMessage: String?
    let unlock: () -> Void

    var body: some View {
        GroupBox {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: "lock.shield.fill")
                    .font(.title2)
                    .foregroundStyle(.orange)

                VStack(alignment: .leading, spacing: 8) {
                    Text("Local vault locked")
                        .font(.headline)
                    Text("Scanning remains read-only, but Rust will not create a Zone, issue an approval, execute, or undo until the local vault is unlocked.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                    HStack {
                        SecureField("Local password", text: $password)
                            .textContentType(.password)
                            .frame(maxWidth: 280)
                            .onSubmit(unlock)
                        Button("Unlock", action: unlock)
                            .buttonStyle(.borderedProminent)
                            .disabled(password.isEmpty || isWorking)
                        if isWorking {
                            ProgressView()
                                .controlSize(.small)
                        }
                    }
                    if let errorMessage {
                        Text(errorMessage)
                            .font(.caption)
                            .foregroundStyle(.red)
                            .textSelection(.enabled)
                    }
                }
                Spacer()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

private struct ScanResultView: View {
    let result: ScanResult
    let createPlan: () -> Void
    let canCreatePlan: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Scan complete")
                        .font(.title2.weight(.semibold))
                    Text("\(result.fileCount.formatted()) files · \(result.totalBytes.formatted(.byteCount(style: .file)))")
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Create Review Plan", action: createPlan)
                    .buttonStyle(.borderedProminent)
                    .disabled(!canCreatePlan)
            }

            GroupBox("Boundary to create") {
                Text("The native preview uses the selected folder as both source and destination. Rust may read metadata, create category folders, and move or rename files only inside this boundary. Every mutation still requires this plan's explicit approval.")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }

            List(result.files) { file in
                HStack(spacing: 10) {
                    Image(systemName: "doc")
                        .foregroundStyle(.secondary)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(file.name)
                        Text(file.path)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    Spacer()
                    Text(file.size.formatted(.byteCount(style: .file)))
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 2)
            }
            .listStyle(.inset)
            .frame(minHeight: 280)

            if result.truncated {
                Text("Showing the first \(result.files.count.formatted()) files. The plan covers all scanned files.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }
}

private struct ApprovedPlanView: View {
    let plan: ExecutionPlan?
    let execute: () -> Void
    let startOver: () -> Void

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.shield.fill")
                .font(.system(size: 44))
                .foregroundStyle(.green)
            Text("Plan approved")
                .font(.title2.weight(.semibold))
            Text("Execution will re-plan and compare the canonical hash before touching the filesystem. The token can be used once.")
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 560)
            if let plan {
                Text("\(plan.summary.totalSteps.formatted()) reviewed steps")
                    .font(.callout.monospacedDigit())
            }
            HStack {
                Button("Review Again", action: startOver)
                Button("Execute Approved Plan", action: execute)
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut("e", modifiers: [.command])
            }
        }
        .frame(maxWidth: .infinity, minHeight: 360)
    }
}

private struct ExecutionProgressView: View {
    let progress: ExecutionProgressState

    var body: some View {
        VStack(spacing: 16) {
            ProgressView(value: progress.fraction)
                .frame(maxWidth: 520)
            Text("Executing approved actions")
                .font(.title2.weight(.semibold))
            Text("\(progress.completed.formatted()) of \(progress.total.formatted()) steps")
                .font(.headline.monospacedDigit())
            if let currentStep = progress.currentStep {
                Text(currentStep)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 640)
            }
            HStack(spacing: 16) {
                Label("\(progress.applied)", systemImage: "checkmark.circle")
                Label("\(progress.skipped)", systemImage: "arrow.right.circle")
                Label("\(progress.failed)", systemImage: "xmark.octagon")
            }
            .font(.callout.monospacedDigit())
            .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, minHeight: 360)
    }
}

private struct UndoCompletedView: View {
    let report: UndoReport?
    let startOver: () -> Void

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "arrow.uturn.backward.circle.fill")
                .font(.system(size: 44))
                .foregroundStyle(.green)
            Text("Undo complete")
                .font(.title2.weight(.semibold))
            if let report {
                Text("\(report.reverted) reverted · \(report.skipped) skipped · \(report.failed) failed")
                    .font(.headline.monospacedDigit())
            }
            Text("Ghost never overwrites an occupied original path and never removes a folder that is no longer empty.")
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 560)
            Button("Start Over", action: startOver)
                .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, minHeight: 360)
    }
}

private struct FailureView: View {
    let message: String
    let retry: () -> Void

    var body: some View {
        VStack(spacing: 14) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 42))
                .foregroundStyle(.orange)
            Text("Ghost stopped safely")
                .font(.title2.weight(.semibold))
            Text(message)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .textSelection(.enabled)
                .frame(maxWidth: 620)
            Button("Return to Folder", action: retry)
                .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, minHeight: 360)
    }
}
