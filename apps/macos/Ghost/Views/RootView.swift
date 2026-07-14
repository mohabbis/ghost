import SwiftUI

struct RootView: View {
    @ObservedObject var environment: AppEnvironment
    @SceneStorage("ghost.native.selectedSection") private var selectedRawValue = AppSection.organizer.rawValue

    private var selection: Binding<AppSection> {
        Binding(
            get: { AppSection(rawValue: selectedRawValue) ?? .organizer },
            set: { selectedRawValue = $0.rawValue }
        )
    }

    var body: some View {
        NavigationSplitView {
            SidebarView(selection: selection)
                .navigationSplitViewColumnWidth(min: 180, ideal: 220, max: 280)
        } detail: {
            switch selection.wrappedValue {
            case .organizer:
                OrganizerView(environment: environment)
            case .history:
                HistoryView(core: environment.core)
            }
        }
    }
}
