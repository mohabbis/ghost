import ApplicationServices
import Combine
import Foundation

@MainActor
final class PermissionService: ObservableObject {
    @Published private(set) var accessibilityTrusted = AXIsProcessTrusted()

    func refresh() {
        accessibilityTrusted = AXIsProcessTrusted()
    }

    func requestAccessibility() {
        let promptKey = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
        let options = [promptKey: true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
        refresh()
    }
}
