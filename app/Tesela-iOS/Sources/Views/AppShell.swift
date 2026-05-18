import SwiftUI

/// Top-level scaffold. Custom bottom chrome with three Liquid Glass
/// shapes on one row (mockup E): tab capsule (Daily · Inbox · Library)
/// on the left, search circle and capture circle on the right.
///
/// We render the bottom bar ourselves rather than using SwiftUI's
/// `TabView` because the native tab bar pill auto-expands to nearly
/// the full screen width, which causes overlap with floating buttons.
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

    private var shell: some View {
        ZStack(alignment: .bottom) {
            activeContent
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(appearance.theme.bg)
                .ignoresSafeArea(.container, edges: .bottom)
            BottomChrome(
                activeTab: $activeTab,
                onSearch: { showSearch = true },
                onCapture: { showCapture = true }
            )
        }
    }

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

// MARK: - Bottom chrome (mockup E)

/// Tab capsule on the left, search circle + capture circle on the
/// right. Each is its own Liquid Glass shape; layout is a real HStack
/// so the tab capsule is sized to its content rather than expanding
/// over the action buttons.
private struct BottomChrome: View {
    @Binding var activeTab: AppTab
    let onSearch: () -> Void
    let onCapture: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 8) {
            tabCapsule
            Spacer(minLength: 6)
            actionCircle(systemImage: "magnifyingglass", label: "Search", action: onSearch)
            actionCircle(systemImage: "plus", label: "Capture", tint: theme.accentPrimary, action: onCapture)
        }
        .padding(.horizontal, 14)
        .padding(.bottom, 16)
    }

    /// Group 1 — three tab buttons in one glass capsule.
    private var tabCapsule: some View {
        HStack(spacing: 2) {
            ForEach(AppTab.allCases) { tab in
                tabButton(tab)
            }
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 5)
        .glassEffect(.regular.interactive(), in: .capsule)
    }

    private func tabButton(_ tab: AppTab) -> some View {
        let on = (activeTab == tab)
        return Button {
            withAnimation(.snappy(duration: 0.18)) { activeTab = tab }
        } label: {
            VStack(spacing: 2) {
                Image(systemName: tab.systemImage)
                    .font(.system(size: 17, weight: on ? .semibold : .regular))
                Text(tab.label)
                    .font(.system(size: 10, weight: on ? .semibold : .regular))
            }
            .foregroundStyle(on ? theme.accentPrimary : theme.fgMuted)
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .frame(minWidth: 56)
            .background {
                if on {
                    Capsule().fill(theme.accentPrimary.opacity(0.16))
                }
            }
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
    }

    /// Single Liquid Glass circle for an action button. Optional tint
    /// for the brand-tinted capture button.
    private func actionCircle(
        systemImage: String,
        label: String,
        tint: Color? = nil,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 18, weight: .semibold))
                .frame(width: 48, height: 48)
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .glassEffect(
            tint.map { Glass.regular.tint($0).interactive() } ?? .regular.interactive(),
            in: .circle
        )
        .accessibilityLabel(label)
    }
}

// MARK: - Tab SF-Symbol mapping

extension AppTab {
    /// SF Symbol name for the tab's glyph. Native system icons on
    /// iOS per Taylor's "use the SF equivalents" direction.
    var systemImage: String {
        switch self {
        case .daily:   return "calendar"
        case .inbox:   return "tray"
        case .library: return "doc.text"
        }
    }
}
