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

            // iOS 26 renders a Tab with `role: .search` as a SEPARATE
            // floating glass chip alongside the main tab group — exactly
            // the "Single" treatment Taylor wants. Tapping this chip
            // enters the search experience; we route to the SearchView
            // sheet rather than letting iOS use the default expansion.
            Tab(value: AppTab.search, role: .search) {
                SearchView(mosaic: mosaic, pageStack: pageStack, syncState: syncState)
            } label: {
                TabBarLabel(tab: .search, active: activeTab == .search)
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        // Capture button floats as a third glass shape at the same Y as
        // the tab bar group + search chip, right-aligned over the tab
        // bar's safe-area inset. iOS 26 native tab bar handles the
        // first two groups (tabs + search chip); we provide the third.
        .overlay(alignment: .bottomTrailing) {
            CaptureGlassButton { showCapture = true }
                .padding(.trailing, 16)
                .padding(.bottom, 14)
        }
    }
}

/// Single brand-tinted Liquid Glass circle for capture. Floats at the
/// bottom-trailing edge so it sits on the same line as the system tab
/// bar group + search chip — three independent glass shapes on one row.
private struct CaptureGlassButton: View {
    let action: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        Button(action: action) {
            Icon(name: .plus, size: 20, lineWidth: 2.2)
                .foregroundStyle(theme.fgDefault)
                .frame(width: 44, height: 44)
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .glassEffect(.regular.tint(theme.accentPrimary).interactive(), in: .circle)
        .accessibilityLabel("Capture")
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

