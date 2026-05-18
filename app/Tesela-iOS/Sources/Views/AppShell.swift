import SwiftUI

/// Top-level three-tab scaffold. Only Daily is wired in Phase 4;
/// Library and Search are empty-state placeholders that get filled
/// in later phases.
struct AppShell: View {
    @StateObject private var appearance = AppearanceController()
    @StateObject private var mosaic = MockMosaicService()
    @State private var activeTab: AppTab = .daily
    @State private var captureText: String = ""

    @Environment(\.theme) private var environmentTheme  // injected by TeselaAppearance

    var body: some View {
        TeselaAppearance(controller: appearance) {
            shell
                .ignoresSafeArea(.keyboard, edges: .bottom)
        }
    }

    @ViewBuilder
    private var shell: some View {
        GeometryReader { proxy in
            ZStack {
                appearance.theme.bg.ignoresSafeArea()
                VStack(spacing: 0) {
                    // Active tab body
                    Group {
                        switch activeTab {
                        case .daily:
                            DailyView(mosaic: mosaic)
                        case .library:
                            placeholderView(
                                title: "Library",
                                hint: "Phase 5 — flat list + type-filter strip"
                            )
                        case .search:
                            placeholderView(
                                title: "Search",
                                hint: "Phase 7 — fused with capture-bar palette mode"
                            )
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

                    CaptureBar(
                        text: $captureText,
                        onSend: { sendCapture() },
                        onMic: { /* Phase 9 placeholder */ }
                    )

                    BottomTabBar(active: $activeTab)
                }
            }
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
    }

    private func sendCapture() {
        mosaic.capture(captureText)
        captureText = ""
        // Snap back to Daily so the user sees their new block.
        activeTab = .daily
    }
}
