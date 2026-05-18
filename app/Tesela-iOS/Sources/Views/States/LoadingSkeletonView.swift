import SwiftUI

/// Loading skeleton used before the mosaic loads (mock layer or FFI).
/// Animated shimmer bars suggest body / list rows. Matches the canvas's
/// S3 loading screen.
struct LoadingSkeletonView: View {
    @Environment(\.theme) private var theme
    @State private var phase: CGFloat = 0

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 12) {
                shimmerBar(width: 0.4, height: 14)
                shimmerBar(width: 0.74, height: 14)
                shimmerBar(width: 0.5, height: 14)
            }
            .padding(.horizontal, 18)
            .padding(.top, 16)
            .padding(.bottom, 8)

            ForEach(0..<8, id: \.self) { i in
                HStack(spacing: 10) {
                    Text("·")
                        .foregroundStyle(theme.fgFaint)
                        .frame(width: 14)
                    shimmerBar(width: rowWidth(i), height: 12)
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 6)
            }
            Spacer()
            HStack {
                Text("indexing · 14k blocks")
                Spacer()
                Text("via tesela-core uniffi")
            }
            .font(.system(size: 10.5, design: .monospaced))
            .foregroundStyle(theme.fgFaint)
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(theme.bg2)
            .overlay(alignment: .top) {
                Rectangle().fill(theme.line).frame(height: 1)
            }
        }
        .background(theme.bg)
        .onAppear {
            withAnimation(.linear(duration: 1.6).repeatForever(autoreverses: false)) {
                phase = 1
            }
        }
    }

    private func rowWidth(_ i: Int) -> CGFloat {
        let widths: [CGFloat] = [0.80, 0.90, 0.60, 0.75, 0.92, 0.55, 0.88, 0.70]
        return widths[i % widths.count]
    }

    private func shimmerBar(width: CGFloat, height: CGFloat) -> some View {
        GeometryReader { geo in
            let w = geo.size.width * width
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 3)
                    .fill(theme.bg2)
                    .frame(width: w, height: height)
                LinearGradient(
                    colors: [theme.bg2, theme.bg3, theme.bg2],
                    startPoint: .leading,
                    endPoint: .trailing
                )
                .frame(width: w, height: height)
                .mask {
                    RoundedRectangle(cornerRadius: 3).frame(width: w, height: height)
                }
                .offset(x: phase * (w - 60) - 30)
            }
        }
        .frame(height: height)
    }
}
