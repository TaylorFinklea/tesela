import SwiftUI

/// Top-level scaffold. Three Liquid Glass groups along the bottom on a
/// single horizontal row: tab group (Daily · Inbox · Library), a
/// search singleton, and a capture singleton. Each is its own
/// `GlassEffectContainer`, side-by-side as proper layout siblings —
/// no overlay tricks.
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
                shell
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

    /// The main screen: active tab content + a bottom chrome row.
    private var shell: some View {
        ZStack(alignment: .bottom) {
            activeContent
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(appearance.theme.bg)
            BottomChrome(
                activeTab: $activeTab,
                onSearch: { showSearch = true },
                onCapture: { showCapture = true }
            )
        }
    }

    /// Switch on the active tab to render that tab's content. Done as
    /// a `switch` rather than via `TabView` so the bottom chrome below
    /// is fully custom — we own the layout, not iOS's tab-bar engine.
    @ViewBuilder
    private var activeContent: some View {
        switch activeTab {
        case .daily:
            DailyView(mosaic: mosaic, backend: backend)
        case .inbox:
            InboxView(mosaic: mosaic, backend: backend)
        case .library:
            LibraryView(
                mosaic: mosaic,
                appearance: appearance,
                pageStack: pageStack,
                syncState: syncState,
                backend: backend
            )
        }
    }
}

// MARK: - Bottom chrome — three sibling Liquid Glass groups

/// Renders the three glass groups: tab pill on the left, search on
/// the right, capture on the far right. Each lives inside its own
/// `GlassEffectContainer` so SwiftUI renders them as distinct
/// floating shapes (rather than merging them into one bar).
private struct BottomChrome: View {
    @Binding var activeTab: AppTab
    let onSearch: () -> Void
    let onCapture: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 10) {
            tabGroup
            Spacer(minLength: 8)
            searchGroup
            captureGroup
        }
        .padding(.horizontal, 14)
        .padding(.bottom, 16)
    }

    /// Group 1 — the tab pill containing Daily · Inbox · Library.
    private var tabGroup: some View {
        GlassEffectContainer(spacing: 0) {
            HStack(spacing: 4) {
                ForEach(AppTab.allCases) { tab in
                    tabButton(tab)
                }
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 4)
            .glassEffect(.regular.interactive(), in: .capsule)
        }
    }

    private func tabButton(_ tab: AppTab) -> some View {
        let on = (activeTab == tab)
        return Button {
            withAnimation(.snappy(duration: 0.18)) { activeTab = tab }
        } label: {
            VStack(spacing: 1) {
                Icon(name: tab.icon, size: 18)
                Text(tab.label)
                    .font(.system(size: 10, weight: on ? .semibold : .regular, design: .default))
                    .tracking(0)
            }
            .foregroundStyle(on ? theme.accentPrimary : theme.fgMuted)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .frame(minWidth: 60)
            .background {
                if on {
                    Capsule()
                        .fill(theme.accentPrimary.opacity(0.16))
                }
            }
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
    }

    /// Group 2 — search singleton glass circle.
    private var searchGroup: some View {
        GlassEffectContainer(spacing: 0) {
            Button(action: onSearch) {
                Icon(name: .search, size: 20, lineWidth: 2)
                    .foregroundStyle(theme.fgDefault)
                    .frame(width: 48, height: 48)
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .glassEffect(.regular.interactive(), in: .circle)
            .accessibilityLabel("Search")
        }
    }

    /// Group 3 — capture singleton glass circle, brand-tinted.
    private var captureGroup: some View {
        GlassEffectContainer(spacing: 0) {
            Button(action: onCapture) {
                Icon(name: .plus, size: 22, lineWidth: 2.2)
                    .foregroundStyle(theme.fgDefault)
                    .frame(width: 48, height: 48)
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .glassEffect(.regular.tint(theme.accentPrimary).interactive(), in: .circle)
            .accessibilityLabel("Capture")
        }
    }
}
