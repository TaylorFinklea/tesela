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
        // Group 2 — search+capture capsule. Same visual treatment as
        // the system tab bar group (Daily/Inbox/Library) but as a
        // separate Liquid Glass capsule. Floats at the bottom-trailing
        // edge so it sits on the same row as the tab bar.
        .overlay(alignment: .bottomTrailing) {
            SearchCaptureCapsule(
                onTapSearch: { showSearch = true },
                onTapCapture: { showCapture = true }
            )
            .padding(.trailing, 16)
            .padding(.bottom, 14)
        }
        .sheet(isPresented: $showSearch) {
            SearchView(mosaic: mosaic, pageStack: pageStack, syncState: syncState)
                .environment(\.theme, appearance.theme)
                .environment(\.density, appearance.density)
                .presentationDetents([.large])
        }
    }
}

/// Group 2 — a Liquid Glass capsule containing two action buttons,
/// rendered identically to the tab bar group (Daily/Inbox/Library) but
/// as a separate floating capsule. Search on the left, Capture on the
/// right. Same shape, same glass treatment, same height.
private struct SearchCaptureCapsule: View {
    let onTapSearch: () -> Void
    let onTapCapture: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 0) {
            actionButton(icon: .search, action: onTapSearch, label: "Search")
            actionButton(icon: .plus, action: onTapCapture, label: "Capture", tinted: true)
        }
        .glassEffect(.regular.interactive(), in: .capsule)
    }

    private func actionButton(
        icon: IconName,
        action: @escaping () -> Void,
        label: String,
        tinted: Bool = false
    ) -> some View {
        Button(action: action) {
            Icon(name: icon, size: 20, lineWidth: 2)
                .foregroundStyle(tinted ? theme.accentPrimary : theme.fgDefault)
                .frame(width: 50, height: 44)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
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

