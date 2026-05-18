import SwiftUI

/// Top-level scaffold using iOS 26's native Liquid Glass `TabView`.
/// Three "place" tabs: Daily · Inbox · Library. Search and capture
/// are NOT tabs — both live in `.tabViewBottomAccessory` as a Mail-
/// style search field with an adjacent + capture button.
struct AppShell: View {
    @StateObject private var appearance = AppearanceController()
    @StateObject private var mosaic = MockMosaicService()
    @StateObject private var pageStack = PageStack()
    @StateObject private var syncState = SyncState()
    @StateObject private var backend = BackendSettings()
    @State private var activeTab: AppTab = .daily
    @State private var showCapture: Bool = false
    @State private var showSearch: Bool = false
    @State private var captureSeed: String = ""

    @AppStorage("onboardingComplete") private var onboardingComplete: Bool = false

    var body: some View {
        TeselaAppearance(controller: appearance) {
            if onboardingComplete {
                tabView
                    .sheet(isPresented: $showCapture) {
                        CaptureSheet(mosaic: mosaic, seed: captureSeed)
                            .environment(\.theme, appearance.theme)
                            .environment(\.density, appearance.density)
                            .onDisappear { captureSeed = "" }
                    }
                    .sheet(isPresented: $showSearch) {
                        SearchView(mosaic: mosaic, pageStack: pageStack, syncState: syncState)
                            .environment(\.theme, appearance.theme)
                            .environment(\.density, appearance.density)
                            .presentationDetents([.large])
                    }
                    .task {
                        mosaic.attach(backend: backend.backend)
                        await mosaic.refresh(from: backend.backend)
                    }
            } else {
                OnboardingView(onboardingComplete: $onboardingComplete)
            }
        }
    }

    private var tabView: some View {
        TabView(selection: $activeTab) {
            Tab(value: AppTab.daily) {
                DailyView(mosaic: mosaic, backend: backend)
            } label: {
                TabBarLabel(tab: .daily, active: activeTab == .daily)
            }

            Tab(value: AppTab.inbox) {
                InboxView(mosaic: mosaic, backend: backend)
            } label: {
                TabBarLabel(tab: .inbox, active: activeTab == .inbox)
            }

            Tab(value: AppTab.library) {
                LibraryView(
                    mosaic: mosaic,
                    appearance: appearance,
                    pageStack: pageStack,
                    syncState: syncState,
                    backend: backend
                )
            } label: {
                TabBarLabel(tab: .library, active: activeTab == .library)
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        .tabViewBottomAccessory {
            SearchAndCaptureAccessory(
                onTapSearch: { showSearch = true },
                onTapCapture: { showCapture = true }
            )
        }
    }
}

/// Label view used inside each `Tab` — pairs a Tabler-shaped icon with
/// the tab's text. The native tab bar handles selection styling
/// (`active` is passed through so we can tune the icon contrast if the
/// system rendering needs it later).
private struct TabBarLabel: View {
    let tab: AppTab
    let active: Bool

    var body: some View {
        Label {
            Text(tab.label)
        } icon: {
            Icon(name: tab.icon, size: 22)
        }
    }
}

/// Mail-style search bar with an adjacent + capture button. Replaces
/// the earlier always-visible capture pill. The search field is
/// tappable — taps open the SearchView as a sheet so the keyboard +
/// results appear over the active tab without leaving it.
private struct SearchAndCaptureAccessory: View {
    let onTapSearch: () -> Void
    let onTapCapture: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 10) {
            searchField
            captureButton
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
    }

    private var searchField: some View {
        Button(action: onTapSearch) {
            HStack(spacing: 8) {
                Icon(name: .search, size: 16)
                    .foregroundStyle(theme.fgSubtle)
                Text("Search")
                    .font(.system(size: 15))
                    .foregroundStyle(theme.fgFaint)
                Spacer(minLength: 0)
                Icon(name: .mic, size: 16)
                    .foregroundStyle(theme.fgSubtle)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(
                Capsule()
                    .fill(theme.bg3.opacity(0.6))
            )
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Search")
    }

    private var captureButton: some View {
        Button(action: onTapCapture) {
            Icon(name: .plus, size: 18, lineWidth: 2)
                .foregroundStyle(theme.bg)
                .frame(width: 34, height: 34)
                .background(
                    Circle().fill(theme.accentPrimary)
                )
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Capture")
    }
}
