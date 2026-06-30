import SwiftUI

/// First-launch onboarding. Per decision #3: "Join existing mosaic" is
/// the **primary** CTA — Taylor's only realistic onboarding is the
/// desktop-already-exists migration path. "Create a new mosaic" is a
/// quieter secondary button.
///
/// Gated by `@AppStorage("onboardingComplete")`; once flipped, the
/// app launches into `AppShell` directly.
struct OnboardingView: View {
    @Binding var onboardingComplete: Bool
    @ObservedObject var backend: BackendSettings
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var registry: MosaicRegistry
    @State private var showPair: Bool = false
    /// Drives the "you're synced" confirmation push — set by
    /// `PairDeviceView`'s `onPaired` signal, which only this onboarding
    /// call site wires up (Settings' two call sites leave it `nil`, so
    /// they never see this screen). Mirrors `showPair`'s
    /// `navigationDestination(isPresented:)` idiom one level deeper.
    @State private var showSyncedConfirmation: Bool = false
    /// Inviter/mosaic name carried up from `PairingCodeRecord.displayName`
    /// via the QR-scan or short-code adopt() call; `nil`/blank falls back
    /// to generic copy in `OnboardingConfirmationView`.
    @State private var pairedInviterName: String? = nil

    @Environment(\.theme) private var theme

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 0) {
                brandMark
                title
                subtitle
                bulletList
                Spacer()
                ctas
            }
            .padding(.horizontal, 28)
            .padding(.top, 36)
            .padding(.bottom, 36)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(theme.bg)
            .navigationDestination(isPresented: $showPair) {
                PairDeviceView(
                    backend: backend, mosaic: mosaic, registry: registry,
                    onPaired: { name in
                        pairedInviterName = name
                        showSyncedConfirmation = true
                    }
                )
                .environment(\.theme, theme)
                .navigationDestination(isPresented: $showSyncedConfirmation) {
                    OnboardingConfirmationView(inviterName: pairedInviterName) {
                        onboardingComplete = true
                    }
                    .environment(\.theme, theme)
                }
            }
        }
    }

    // ── Brand mark ──────────────────────────────────────────────────────

    private var brandMark: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 14)
                .fill(
                    LinearGradient(
                        colors: [theme.bg3, theme.bg2],
                        startPoint: .top,
                        endPoint: .bottom
                    )
                )
                .frame(width: 56, height: 56)
                .overlay(
                    RoundedRectangle(cornerRadius: 14)
                        .stroke(theme.line, lineWidth: 1)
                )
            Text("t")
                .font(.system(size: 26, weight: .bold, design: .monospaced))
                .foregroundStyle(theme.accentPrimary)
        }
        .padding(.bottom, 24)
    }

    private var title: some View {
        Text("Tesela on iPhone")
            .font(.system(size: 32, weight: .semibold))
            .tracking(-0.48)
            .foregroundStyle(theme.fgDefault)
            .padding(.top, 4)
    }

    private var subtitle: some View {
        Text("Your mosaic, in your pocket. Daily notes and quick capture at the front. Pages and tags are a tap away.")
            .font(.system(size: 16))
            .foregroundStyle(theme.fgMuted)
            .lineSpacing(3)
            .padding(.top, 12)
    }

    // ── Bullets ─────────────────────────────────────────────────────────

    private var bulletList: some View {
        VStack(alignment: .leading, spacing: 14) {
            bullet(
                icon: .bolt,
                title: "Local-first",
                body: "Everything is markdown on your device. Sync is peer-to-peer when the app is open."
            )
            bullet(
                icon: .share,
                title: "Capture from anywhere",
                body: "Share-sheet, Shortcuts, and quick capture all land in today's daily."
            )
            bullet(
                icon: .sync,
                title: "Same mosaic",
                body: "Sees the same files as the web client and (paused) macOS app."
            )
        }
        .padding(.top, 28)
    }

    private func bullet(icon: IconName, title: String, body: String) -> some View {
        HStack(alignment: .top, spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 7)
                    .fill(theme.accentPrimary.opacity(0.14))
                    .frame(width: 28, height: 28)
                Icon(name: icon, size: 16)
                    .foregroundStyle(theme.accentPrimary)
            }
            .padding(.top, 2)

            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.system(size: 14.5, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)
                Text(body)
                    .font(.system(size: 13))
                    .foregroundStyle(theme.fgSubtle)
                    .lineSpacing(2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
        }
    }

    // ── CTAs ────────────────────────────────────────────────────────────

    private var ctas: some View {
        VStack(spacing: 10) {
            Button {
                showPair = true
            } label: {
                Text("Join existing mosaic")
                    .font(.system(size: 16, weight: .semibold))
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .foregroundStyle(theme.bg)
                    .background(theme.accentPrimary)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .buttonStyle(.plain)

            Button {
                onboardingComplete = true
            } label: {
                Text("Create a new mosaic")
                    .font(.system(size: 14))
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
                    .foregroundStyle(theme.fgMuted)
            }
            .buttonStyle(.plain)
        }
    }
}

/// Calm "you're synced" success screen pushed after a pair succeeds
/// during onboarding — and ONLY during onboarding; `PairDeviceView`'s
/// `onPaired` signal is `nil` (a no-op) at both Settings call sites, so
/// this view is unreachable from there. Mirrors `OnboardingView`'s own
/// brand-mark / title / subtitle / single-CTA layout rather than a
/// system alert. `onboardingComplete` is flipped exactly once, by the
/// explicit "Continue" tap below.
struct OnboardingConfirmationView: View {
    /// Inviter/mosaic display name when available (threaded down from
    /// `PairingCodeRecord.displayName` on both the QR-scan and
    /// short-code paths). Blank/whitespace-only collapses to `nil` so we
    /// never render an empty or garbled name.
    let inviterName: String?
    let onContinue: () -> Void

    @Environment(\.theme) private var theme

    /// Pure trim/collapse: blank or whitespace-only names fall back to
    /// `nil` so the confirmation never renders an empty/garbled name.
    /// Static + pure (mirrors `PairDeviceView.displayState(for:)`) so
    /// it's directly testable from `PairingRoutingTests` without
    /// standing up a view.
    static func resolvedInviterName(from raw: String?) -> String? {
        guard let raw else { return nil }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private var trimmedInviterName: String? {
        Self.resolvedInviterName(from: inviterName)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            successMark
            title
            subtitle
            Spacer()
            continueButton
        }
        .padding(.horizontal, 28)
        .padding(.top, 36)
        .padding(.bottom, 36)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.bg)
        .navigationBarBackButtonHidden(true)
    }

    private var successMark: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 14)
                .fill(
                    LinearGradient(
                        colors: [theme.bg3, theme.bg2],
                        startPoint: .top,
                        endPoint: .bottom
                    )
                )
                .frame(width: 56, height: 56)
                .overlay(
                    RoundedRectangle(cornerRadius: 14)
                        .stroke(theme.line, lineWidth: 1)
                )
            Icon(name: .check, size: 24)
                .foregroundStyle(theme.accentPrimary)
        }
        .padding(.bottom, 24)
    }

    private var title: some View {
        Text("You're synced")
            .font(.system(size: 32, weight: .semibold))
            .tracking(-0.48)
            .foregroundStyle(theme.fgDefault)
            .padding(.top, 4)
    }

    private var subtitle: some View {
        Text(subtitleCopy)
            .font(.system(size: 16))
            .foregroundStyle(theme.fgMuted)
            .lineSpacing(3)
            .padding(.top, 12)
    }

    private var subtitleCopy: String {
        if let trimmedInviterName {
            return "This iPhone is now connected to \(trimmedInviterName). Your notes will keep in sync in the background."
        }
        return "This iPhone is connected and syncing in the background."
    }

    private var continueButton: some View {
        Button(action: onContinue) {
            Text("Continue")
                .font(.system(size: 16, weight: .semibold))
                .frame(maxWidth: .infinity)
                .padding(.vertical, 14)
                .foregroundStyle(theme.bg)
                .background(theme.accentPrimary)
                .clipShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
    }
}
