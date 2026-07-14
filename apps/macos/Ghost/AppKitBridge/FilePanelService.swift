import AppKit
import Foundation

@MainActor
final class FilePanelService {
    func chooseFolder() -> URL? {
        let panel = NSOpenPanel()
        panel.title = "Choose a folder for Ghost Organizer"
        panel.message = "Ghost reads metadata only during scanning. Nothing changes until you review and approve a plan."
        panel.prompt = "Choose Folder"
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = false
        panel.resolvesAliases = true
        return panel.runModal() == .OK ? panel.url : nil
    }
}
