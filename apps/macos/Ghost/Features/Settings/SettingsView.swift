import SwiftUI

struct SettingsView: View {
    @ObservedObject private var environment: AppEnvironment
    @ObservedObject private var permissions: PermissionService
    @State private var password = ""

    init(environment: AppEnvironment) {
        _environment = ObservedObject(wrappedValue: environment)
        _permissions = ObservedObject(wrappedValue: environment.permissions)
    }

    var body: some View {
        Form {
            Section("Rust execution core") {
                switch environment.bridgeState {
                case .connecting:
                    LabeledContent("Status") {
                        ProgressView()
                            .controlSize(.small)
                    }
                case .connected(let handshake):
                    LabeledContent("Status", value: "Connected")
                    LabeledContent("Protocol", value: handshake.protocolVersion.formatted())
                    LabeledContent("Core version", value: handshake.coreVersion)
                    LabeledContent("Capabilities", value: handshake.capabilities.joined(separator: ", "))
                case .unavailable(let message):
                    LabeledContent("Status", value: "Unavailable")
                    Text(message)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                    Button("Reconnect", action: environment.reconnect)
                }
            }

            Section("Local vault") {
                LabeledContent(
                    "Status",
                    value: vaultStatusLabel
                )

                if environment.authStatus.configured && !environment.authStatus.unlocked {
                    SecureField("Local password", text: $password)
                        .textContentType(.password)
                        .onSubmit(unlock)
                    Button("Unlock", action: unlock)
                        .buttonStyle(.borderedProminent)
                        .disabled(password.isEmpty || environment.isUpdatingVault)
                } else if environment.authStatus.configured {
                    Button("Lock Now") {
                        Task { await environment.lockVault() }
                    }
                    .disabled(environment.isUpdatingVault)
                } else {
                    Text("No local vault password is configured. The native preview follows the existing Ghost behavior and remains usable, although setting up vault protection in the Tauri app is recommended.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                if environment.isUpdatingVault {
                    ProgressView()
                        .controlSize(.small)
                }
                if let error = environment.vaultError {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .textSelection(.enabled)
                }
            }

            Section("macOS permissions") {
                LabeledContent(
                    "Accessibility",
                    value: permissions.accessibilityTrusted ? "Granted" : "Not granted"
                )
                Text("Organizer does not need Accessibility permission. It becomes relevant when native Routines and semantic UI targeting are added.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                HStack {
                    Button("Refresh", action: permissions.refresh)
                    Button("Request Accessibility", action: permissions.requestAccessibility)
                        .disabled(permissions.accessibilityTrusted)
                }
            }

            Section("Trust boundary") {
                Text("SwiftUI presents and coordinates. Rust validates policy, signs approvals, mutates files, writes audit records, and owns undo journals. The native UI never performs trusted filesystem mutations directly.")
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .padding(16)
        .frame(width: 580, height: 600)
    }

    private var vaultStatusLabel: String {
        if !environment.authStatus.configured {
            return "Not configured"
        }
        return environment.authStatus.unlocked ? "Unlocked" : "Locked"
    }

    private func unlock() {
        let candidate = password
        Task {
            if await environment.unlockVault(password: candidate) {
                password = ""
            }
        }
    }
}
