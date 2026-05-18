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
            Tab("Daily", systemImage: "calendar", value: AppTab.daily) {
                DailyView(mosaic: mosaic, backend: backend)
            }
            Tab("Inbox", systemImage: "tray", value: AppTab.inbox) {
                InboxView(mosaic: mosaic, backend: backend)
            }
            Tab("Library", systemImage: "doc.text", value: AppTab.library) {
                LibraryView(
                    mosaic: mosaic,
                    appearance: appearance,
                    pageStack: pageStack,
                    syncState: syncState,
                    backend: backend
                )
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        // Mockup variant E — two single Liquid Glass circles floating
        // at bottom-trailing, on the same row as the tab bar group.
        // Search circle (untinted) + Capture circle (brand-tinted).
        .overlay(alignment: .bottomTrailing) {
            HStack(spacing: 8) {
                SingleGlassButton(
                    systemImage: "magnifyingglass",
                    accessibilityLabel: "Search",
                    action: { showSearch = true }
                )
                SingleGlassButton(
                    systemImage: "plus",
                    accessibilityLabel: "Capture",
                    tint: appearance.theme.accentPrimary,
                    action: { showCapture = true }
                )
            }
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

/// One Liquid Glass circle button. Used for the search and capture
/// singletons. SF Symbol inside (system-rendered glyph) per Taylor's
/// "use the SF equivalents on iOS" direction.
private struct SingleGlassButton: View {
    let systemImage: String
    let accessibilityLabel: String
    var tint: Color? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 18, weight: .semibold))
                .frame(width: 48, height: 48)
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .glassEffect(glassStyle, in: .circle)
        .accessibilityLabel(accessibilityLabel)
    }

    private var glassStyle: Glass {
        if let tint {
            return .regular.tint(tint).interactive()
        }
        return .regular.interactive()
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

