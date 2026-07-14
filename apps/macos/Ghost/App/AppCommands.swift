import SwiftUI

struct GhostAppCommands: Commands {
    @ObservedObject var organizer: OrganizerStore

    var body: some Commands {
        CommandGroup(after: .newItem) {
            Button("Choose Organizer Folder…") {
                organizer.chooseFolder()
            }
            .keyboardShortcut("o", modifiers: [.command])
            .disabled(organizer.isBusy)
        }

        CommandMenu("Organizer") {
            Button("Scan Folder") {
                organizer.scan()
            }
            .keyboardShortcut("r", modifiers: [.command])
            .disabled(!organizer.canScan)

            Button("Create Review Plan") {
                organizer.createPlan()
            }
            .disabled(!organizer.canCreatePlan)

            Divider()

            Button("Approve Plan") {
                organizer.approvePlan()
            }
            .keyboardShortcut(.return, modifiers: [.command])
            .disabled(!organizer.canApprove)

            Button("Execute Approved Plan") {
                organizer.executeApprovedPlan()
            }
            .keyboardShortcut("e", modifiers: [.command])
            .disabled(!organizer.canExecute)

            Divider()

            Button("Undo Last Execution") {
                organizer.undoLastExecution()
            }
            .keyboardShortcut("z", modifiers: [.command])
            .disabled(!organizer.canUndo)
        }
    }
}
