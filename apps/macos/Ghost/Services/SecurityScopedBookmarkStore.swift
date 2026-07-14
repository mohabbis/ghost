import Foundation

final class FolderAccessGrant {
    let url: URL
    private let isSecurityScoped: Bool

    init(url: URL) {
        self.url = url
        self.isSecurityScoped = url.startAccessingSecurityScopedResource()
    }

    deinit {
        if isSecurityScoped {
            url.stopAccessingSecurityScopedResource()
        }
    }
}

final class SecurityScopedBookmarkStore {
    private let defaults: UserDefaults
    private let key = "ghost.native.organizer.folderBookmark"

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func grant(for url: URL) throws -> FolderAccessGrant {
        let data = try url.bookmarkData(
            options: [.withSecurityScope],
            includingResourceValuesForKeys: nil,
            relativeTo: nil
        )
        defaults.set(data, forKey: key)
        return FolderAccessGrant(url: url)
    }

    func restoreLastGrant() throws -> FolderAccessGrant? {
        guard let data = defaults.data(forKey: key) else { return nil }
        var stale = false
        let url = try URL(
            resolvingBookmarkData: data,
            options: [.withSecurityScope],
            relativeTo: nil,
            bookmarkDataIsStale: &stale
        )
        if stale {
            let replacement = try url.bookmarkData(
                options: [.withSecurityScope],
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )
            defaults.set(replacement, forKey: key)
        }
        return FolderAccessGrant(url: url)
    }

    func clear() {
        defaults.removeObject(forKey: key)
    }
}
