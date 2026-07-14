import SwiftUI

enum AppSection: String, CaseIterable, Identifiable {
    case organizer
    case history

    var id: String { rawValue }

    var title: String {
        switch self {
        case .organizer: return "Organizer"
        case .history: return "History"
        }
    }

    var systemImage: String {
        switch self {
        case .organizer: return "folder.badge.gearshape"
        case .history: return "clock.arrow.circlepath"
        }
    }
}

struct SidebarView: View {
    @Binding var selection: AppSection

    var body: some View {
        List(AppSection.allCases, selection: $selection) { section in
            Label(section.title, systemImage: section.systemImage)
                .tag(section)
        }
        .listStyle(.sidebar)
        .navigationTitle("Ghost")
    }
}
