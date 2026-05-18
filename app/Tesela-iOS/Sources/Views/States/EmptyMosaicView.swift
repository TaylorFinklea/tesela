import SwiftUI

/// Empty-mosaic state. Shown the first time a user creates a fresh
/// mosaic (not joins existing). Matches the canvas's S2 empty screen.
struct EmptyMosaicView: View {
    var onCapture: () -> Void = {}
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 18) {
            ZStack {
                RoundedRectangle(cornerRadius: 12)
                    .stroke(theme.line, style: StrokeStyle(lineWidth: 1.5, dash: [4, 3]))
                    .frame(width: 64, height: 64)
                Icon(name: .page, size: 28)
                    .foregroundStyle(theme.fgFaint)
            }
            VStack(spacing: 6) {
                Text("A blank page is fine")
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)
                Text("Tap below to drop your first thought. It lands in today's daily — that's where the front door always is.")
                    .font(.system(size: 14))
                    .foregroundStyle(theme.fgSubtle)
                    .multilineTextAlignment(.center)
                    .lineSpacing(3)
                    .padding(.horizontal, 32)
            }
            Button(action: onCapture) {
                HStack(spacing: 8) {
                    Icon(name: .plus, size: 16).foregroundStyle(theme.bg)
                    Text("Capture a thought")
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(theme.bg)
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 10)
                .background(theme.accentPrimary)
                .clipShape(Capsule())
            }
            .buttonStyle(.plain)
            Text("0 pages · 0 tags · ~/Mosaic")
                .font(.system(size: 10.5, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.bg)
    }
}
