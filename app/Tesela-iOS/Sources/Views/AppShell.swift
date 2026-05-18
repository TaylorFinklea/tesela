import SwiftUI

/// Top-level three-tab scaffold using iOS 26's native Liquid Glass
/// `TabView`. The persistent capture trigger lives in
/// `.tabViewBottomAccessory` above the tab bar — tapping it opens
/// `CaptureSheet` rather than switching tabs.
///
/// Custom Tabler icons are supplied via the `Tab(value:role:content:label:)`
/// overload, so we stay off SF Symbols entirely per the brand brief.
struct AppShell: View {
    @StateObject private var appearance = AppearanceController()
    @StateObject private var mosaic = MockMosaicService()
    @State private var activeTab: AppTab = .daily
    @State private var showCapture: Bool = false
    @State private var captureSeed: String = ""

    var body: some View {
        TeselaAppearance(controller: appearance) {
            tabView
                .sheet(isPresented: $showCapture) {
                    CaptureSheet(mosaic: mosaic, seed: captureSeed)
                        .environment(\.theme, appearance.theme)
                        .environment(\.density, appearance.density)
                        .onDisappear { captureSeed = "" }
                }
        }
    }

    private var tabView: some View {
        TabView(selection: $activeTab) {
            Tab(value: AppTab.daily) {
                DailyView(mosaic: mosaic)
            } label: {
                TabBarLabel(tab: .daily, active: activeTab == .daily)
            }

            Tab(value: AppTab.library) {
                LibraryView(mosaic: mosaic)
            } label: {
                TabBarLabel(tab: .library, active: activeTab == .library)
            }

            Tab(value: AppTab.search, role: .search) {
                placeholderView(
                    title: "Search",
                    hint: "Phase 7 — fused with capture-bar palette mode"
                )
            } label: {
                TabBarLabel(tab: .search, active: activeTab == .search)
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        .tabViewBottomAccessory {
            CaptureAccessory(
                seed: $captureSeed,
                onTap: { showCapture = true }
            )
        }
    }

    @ViewBuilder
    private func placeholderView(title: String, hint: String) -> some View {
        VStack(spacing: 10) {
            Text(title)
                .font(.system(size: 22, weight: .semibold))
                .foregroundStyle(appearance.theme.fgDefault)
            Text(hint)
                .font(.system(size: 11.5, design: .monospaced))
                .foregroundStyle(appearance.theme.fgFaint)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(appearance.theme.bg)
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

/// The pill-shaped capture trigger that lives in the tab-view's bottom
/// accessory slot. Visually mirrors the canvas's persistent capture bar
/// (placeholder text on the left, mic affordance on the right) but uses
/// the system's Liquid Glass treatment rather than a hand-built bar.
///
/// Tapping anywhere opens the full `CaptureSheet` modal; the leading
/// `+` button and trailing mic both route through the same `onTap`
/// closure for consistency.
private struct CaptureAccessory: View {
    @Binding var seed: String
    let onTap: () -> Void

    @Environment(\.theme) private var theme
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 10) {
                Icon(name: .plus, size: 18)
                    .foregroundStyle(theme.accentPrimary)
                    .frame(width: 28, height: 28)

                Text("capture to today…")
                    .italic()
                    .font(.system(size: 14))
                    .foregroundStyle(theme.fgFaint)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .lineLimit(1)

                Icon(name: .mic, size: 18)
                    .foregroundStyle(theme.fgSubtle)
                    .frame(width: 28, height: 28)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
