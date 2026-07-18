import AppKit
import ApplicationServices
import Combine
import CoreGraphics
import Foundation

/// OS permissions Ghost may need. Features must degrade when a permission is
/// denied — never a single "grant everything" gate.
/// See `docs/macos-automation-architecture.md`.
enum GhostPermission: String, CaseIterable, Identifiable, Codable {
    case accessibility
    case screenRecording
    case inputMonitoring
    case notifications
    case automation

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .accessibility: return "Accessibility"
        case .screenRecording: return "Screen Recording"
        case .inputMonitoring: return "Input Monitoring"
        case .notifications: return "Notifications"
        case .automation: return "Automation"
        }
    }

    var whyNeeded: String {
        switch self {
        case .accessibility:
            return "Read UI hierarchies and invoke AXPress / set-value on controls."
        case .screenRecording:
            return "Capture window or display frames for OCR and visual fallback."
        case .inputMonitoring:
            return "Observe keyboard and pointer events while recording a routine."
        case .notifications:
            return "Surface local completion and recovery alerts."
        case .automation:
            return "Optional Apple Events / System Events bridges for legacy apps."
        }
    }

    var dependentFeatures: [String] {
        switch self {
        case .accessibility:
            return ["Semantic UI targeting", "Routines AX actions", "Element inspection"]
        case .screenRecording:
            return ["Vision / OCR fallback", "Template capture", "Visual evidence"]
        case .inputMonitoring:
            return ["Routine recording"]
        case .notifications:
            return ["Local alerts"]
        case .automation:
            return ["AppleScript / System Events bridges"]
        }
    }

    var requiresRestart: Bool {
        switch self {
        case .accessibility, .screenRecording, .inputMonitoring:
            return true
        case .notifications, .automation:
            return false
        }
    }

    var degradedBehavior: String {
        switch self {
        case .accessibility:
            return "AX actions unavailable. Organizer and visual analysis may still work."
        case .screenRecording:
            return "AX-only automation remains available. OCR / visual fallback stays off."
        case .inputMonitoring:
            return "Recording disabled. Approved replay may still run where permitted."
        case .notifications:
            return "In-app status only; no system notification banners."
        case .automation:
            return "No Apple Events automation; AX and CG paths unchanged."
        }
    }

    var settingsDeepLink: URL? {
        // Privacy pane anchors — best-effort; macOS may ignore unknown fragments.
        switch self {
        case .accessibility:
            return URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        case .screenRecording:
            return URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        case .inputMonitoring:
            return URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
        case .notifications:
            return URL(string: "x-apple.systempreferences:com.apple.preference.notifications")
        case .automation:
            return URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation")
        }
    }
}

enum PermissionProbeState: String, Codable {
    case granted
    case denied
    /// Probe not implemented or inconclusive on this OS build.
    case unknown
}

struct PermissionRecord: Identifiable, Equatable {
    let permission: GhostPermission
    var state: PermissionProbeState
    var lastValidated: Date?

    var id: String { permission.id }

    var isGranted: Bool { state == .granted }

    var statusLabel: String {
        switch state {
        case .granted: return "Granted"
        case .denied: return "Not granted"
        case .unknown: return "Check System Settings"
        }
    }
}

@MainActor
final class PermissionService: ObservableObject {
    @Published private(set) var records: [PermissionRecord]

    /// Backward-compatible surface used by existing Settings bindings.
    var accessibilityTrusted: Bool {
        records.first(where: { $0.permission == .accessibility })?.isGranted ?? false
    }

    init() {
        records = GhostPermission.allCases.map {
            PermissionRecord(permission: $0, state: .unknown, lastValidated: nil)
        }
        refresh()
    }

    func record(for permission: GhostPermission) -> PermissionRecord {
        records.first(where: { $0.permission == permission })
            ?? PermissionRecord(permission: permission, state: .unknown, lastValidated: nil)
    }

    func refresh() {
        let now = Date()
        records = GhostPermission.allCases.map { permission in
            PermissionRecord(
                permission: permission,
                state: probe(permission),
                lastValidated: now
            )
        }
    }

    func requestAccessibility() {
        let promptKey = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
        let options = [promptKey: true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
        refresh()
    }

    func openSystemSettings(for permission: GhostPermission) {
        if let url = permission.settingsDeepLink {
            NSWorkspace.shared.open(url)
        }
        refresh()
    }

    /// Organizer must remain usable without Input Monitoring or Screen Recording.
    var organizerAvailable: Bool { true }

    var semanticAxAvailable: Bool { accessibilityTrusted }

    var recordingAvailable: Bool {
        record(for: .inputMonitoring).isGranted && accessibilityTrusted
    }

    var visionFallbackAvailable: Bool {
        record(for: .screenRecording).isGranted
    }

    private func probe(_ permission: GhostPermission) -> PermissionProbeState {
        switch permission {
        case .accessibility:
            return AXIsProcessTrusted() ? .granted : .denied
        case .screenRecording:
            // Preflight does not prompt; returns current TCC state.
            return CGPreflightScreenCaptureAccess() ? .granted : .denied
        case .inputMonitoring, .notifications, .automation:
            // No stable public probe without extra frameworks / async APIs yet.
            return .unknown
        }
    }
}
