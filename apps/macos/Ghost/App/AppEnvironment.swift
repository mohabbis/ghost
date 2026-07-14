import Foundation
import Combine

@MainActor
enum BridgeConnectionState: Equatable {
    case connecting
    case connected(CoreHandshake)
    case unavailable(String)
}

@MainActor
final class AppEnvironment: ObservableObject {
    let core: any GhostCoreClient
    let filePanel: FilePanelService
    let bookmarkStore: SecurityScopedBookmarkStore
    let permissions: PermissionService
    let organizer: OrganizerStore

    @Published private(set) var bridgeState: BridgeConnectionState = .connecting
    @Published private(set) var authStatus: AuthStatus = .unconfigured
    @Published private(set) var isUpdatingVault = false
    @Published private(set) var vaultError: String?

    init(
        core: any GhostCoreClient,
        startupError: String? = nil,
        filePanel: FilePanelService = FilePanelService(),
        bookmarkStore: SecurityScopedBookmarkStore = SecurityScopedBookmarkStore(),
        permissions: PermissionService = PermissionService()
    ) {
        self.core = core
        self.filePanel = filePanel
        self.bookmarkStore = bookmarkStore
        self.permissions = permissions
        self.organizer = OrganizerStore(
            core: core,
            filePanel: filePanel,
            bookmarkStore: bookmarkStore
        )

        if let startupError {
            bridgeState = .unavailable(startupError)
        } else {
            connect()
        }
    }

    static func live() -> AppEnvironment {
        do {
            return AppEnvironment(core: try ProcessGhostCoreClient())
        } catch {
            let message = error.localizedDescription
            return AppEnvironment(
                core: UnavailableGhostCoreClient(reason: message),
                startupError: message
            )
        }
    }

    func reconnect() {
        bridgeState = .connecting
        connect()
    }

    @discardableResult
    func unlockVault(password: String) async -> Bool {
        guard !password.isEmpty, !isUpdatingVault else { return false }
        isUpdatingVault = true
        vaultError = nil
        defer { isUpdatingVault = false }

        do {
            authStatus = try await core.unlock(password: password)
            return authStatus.unlocked
        } catch {
            vaultError = error.localizedDescription
            return false
        }
    }

    func lockVault() async {
        guard !isUpdatingVault else { return }
        isUpdatingVault = true
        vaultError = nil
        defer { isUpdatingVault = false }

        do {
            authStatus = try await core.lock()
        } catch {
            vaultError = error.localizedDescription
        }
    }

    func refreshAuthStatus() async {
        do {
            authStatus = try await core.authStatus()
        } catch {
            vaultError = error.localizedDescription
        }
    }

    private func connect() {
        Task { [weak self] in
            guard let self else { return }
            do {
                let handshake = try await core.handshake()
                authStatus = handshake.auth
                bridgeState = .connected(handshake)
            } catch {
                bridgeState = .unavailable(error.localizedDescription)
            }
        }
    }
}
