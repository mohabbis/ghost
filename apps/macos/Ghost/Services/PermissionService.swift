import AppKit
import ApplicationServices
import Combine
import CoreGraphics
import Foundation
import UserNotifications

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

/// Pure mappings from OS probe results → `PermissionProbeState`.
/// Kept free of AppKit / prompting so behavior stays reviewable and testable.
enum PermissionProbeMapping {
    /// IOHIDCheckAccess return values (IOKit): Granted=0, Denied=1, Unknown=2.
    static let ioHidAccessGranted: UInt32 = 0
    static let ioHidAccessDenied: UInt32 = 1
    static let ioHidAccessUnknown: UInt32 = 2

    /// Map `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)` plus optional CG preflight.
    /// Never invents `.granted` from an inconclusive IOKit result unless CG preflight agrees.
    static func inputMonitoring(
        ioHidAccess: UInt32,
        cgPreflightGranted: Bool
    ) -> PermissionProbeState {
        switch ioHidAccess {
        case ioHidAccessGranted:
            return .granted
        case ioHidAccessDenied:
            return .denied
        case ioHidAccessUnknown:
            // IOKit inconclusive. CG preflight is bool-only (no not-determined).
            // If CG says granted, trust that; otherwise stay unknown — do not fake denied
            // when the OS has not made a definitive decision yet.
            return cgPreflightGranted ? .granted : .unknown
        default:
            return .unknown
        }
    }

    /// Map `UNAuthorizationStatus` without prompting.
    /// `.notDetermined` stays `.unknown` so refresh() never looks "denied" before the
    /// user has been asked via Settings UX.
    static func notifications(authorizationStatus: UNAuthorizationStatus) -> PermissionProbeState {
        switch authorizationStatus {
        case .authorized, .provisional, .ephemeral:
            return .granted
        case .denied:
            return .denied
        case .notDetermined:
            return .unknown
        @unknown default:
            // Future statuses: stay unknown — never fake granted.
            return .unknown
        }
    }
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

// MARK: - IOKit Input Monitoring (read-only check; matches diagnose_perms / platform/macos.rs)

/// Listen-event request type — Input Monitoring TCC. Mirrors `kIOHIDRequestTypeListenEvent`.
private let kGhostIOHIDRequestTypeListenEvent: UInt32 = 1

/// Declared via silgen_name so SPM builds do not depend on IOKit header exposure.
/// Returns Granted (0), Denied (1), or Unknown (2). Does not prompt.
@_silgen_name("IOHIDCheckAccess")
private func ghostIOHIDCheckAccess(_ requestType: UInt32) -> UInt32

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
        let previousNotifications = records
            .first(where: { $0.permission == .notifications })?
            .state ?? .unknown

        records = GhostPermission.allCases.map { permission in
            switch permission {
            case .notifications:
                // Keep prior value until the async UNUserNotificationCenter probe returns.
                // Do not call requestAuthorization here — refresh is read-only.
                return PermissionRecord(
                    permission: permission,
                    state: previousNotifications,
                    lastValidated: now
                )
            default:
                return PermissionRecord(
                    permission: permission,
                    state: probe(permission),
                    lastValidated: now
                )
            }
        }
        refreshNotificationsAsync()
    }

    func requestAccessibility() {
        let promptKey = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
        let options = [promptKey: true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
        refresh()
    }

    /// One-shot Input Monitoring prompt via CoreGraphics (does not run from `refresh()`).
    /// Opens System Settings when still not granted — same UX pattern as Accessibility.
    func requestInputMonitoring() {
        _ = CGRequestListenEventAccess()
        if probe(.inputMonitoring) != .granted {
            openSystemSettings(for: .inputMonitoring)
            return
        }
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

    /// Recording needs both Accessibility and a definitive Input Monitoring grant.
    /// `.unknown` / `.denied` keep recording gated off — never optimistic.
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
        case .inputMonitoring:
            return probeInputMonitoring()
        case .notifications:
            // Handled asynchronously in refreshNotificationsAsync().
            return .unknown
        case .automation:
            // No process-wide public probe: Automation TCC is per Apple Event target.
            // Stay unknown rather than fake a grant/deny from a single AE attempt.
            return .unknown
        }
    }

    private func probeInputMonitoring() -> PermissionProbeState {
        // Read-only: IOHIDCheckAccess / CGPreflightListenEventAccess never prompt.
        // Mirrors src-tauri diagnose_perms.rs and platform/macos.rs.
        let ioHid = ghostIOHIDCheckAccess(kGhostIOHIDRequestTypeListenEvent)
        let cgGranted = CGPreflightListenEventAccess()
        return PermissionProbeMapping.inputMonitoring(
            ioHidAccess: ioHid,
            cgPreflightGranted: cgGranted
        )
    }

    private func refreshNotificationsAsync() {
        Task {
            let settings = await UNUserNotificationCenter.current().notificationSettings()
            let state = PermissionProbeMapping.notifications(
                authorizationStatus: settings.authorizationStatus
            )
            await MainActor.run {
                guard let index = records.firstIndex(where: { $0.permission == .notifications })
                else { return }
                var updated = records
                updated[index].state = state
                updated[index].lastValidated = Date()
                records = updated
            }
        }
    }
}
