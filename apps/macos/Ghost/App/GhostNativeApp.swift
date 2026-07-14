import AppKit
import SwiftUI

final class GhostNativeAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }
}

@main
struct GhostNativeApp: App {
    @NSApplicationDelegateAdaptor(GhostNativeAppDelegate.self) private var appDelegate
    @StateObject private var environment: AppEnvironment

    init() {
        _environment = StateObject(wrappedValue: AppEnvironment.live())
    }

    var body: some Scene {
        WindowGroup("Ghost", id: "main") {
            RootView(environment: environment)
                .frame(minWidth: 960, minHeight: 680)
        }
        .defaultSize(width: 1_180, height: 760)
        .commands {
            GhostAppCommands(organizer: environment.organizer)
        }

        Settings {
            SettingsView(environment: environment)
        }
    }
}
