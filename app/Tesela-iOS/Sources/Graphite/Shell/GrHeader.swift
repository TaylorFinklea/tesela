import SwiftUI

/// Graphite per-tab header. Mirrors the mobile mockup's `.grm-head`
/// (`.docs/ai/design/graphite/mobile/grm-shell.jsx`): a large-title row
/// (25pt / weight 650 / tracking -.02em, `fgDefault`) with an optional
/// mono subtitle (11pt, `fgSubtle`), a trailing chrome slot for icon
/// actions, and a hairline bottom border (`lineSoft`).
///
/// Presentation only — reads role colors from `@Environment(\.theme)`
/// like every other Graphite view. The shell supplies the title /
/// subtitle / trailing actions; this view does not own any behavior.
struct GrHeader<Trailing: View>: View {
    let title: String
    var subtitle: String? = nil
    @ViewBuilder var trailing: () -> Trailing

    @Environment(\.theme) private var theme

    init(
        title: String,
        subtitle: String? = nil,
        @ViewBuilder trailing: @escaping () -> Trailing
    ) {
        self.title = title
        self.subtitle = subtitle
        self.trailing = trailing
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .bottom, spacing: 10) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(title)
                        .font(.system(size: 25, weight: .semibold))
                        .tracking(-0.5)
                        .foregroundStyle(theme.fgDefault)
                        .lineLimit(1)
                    if let subtitle {
                        Text(subtitle)
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(theme.fgSubtle)
                            .lineLimit(1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                HStack(spacing: 2) {
                    trailing()
                }
                .padding(.bottom, 2)
            }
            .padding(.leading, 18)
            .padding(.trailing, 14)
            .padding(.top, 6)
            .padding(.bottom, 12)
            Rectangle()
                .fill(theme.lineSoft)
                .frame(height: 1)
        }
    }
}

extension GrHeader where Trailing == EmptyView {
    /// Convenience for a header with no trailing actions.
    init(title: String, subtitle: String? = nil) {
        self.init(title: title, subtitle: subtitle, trailing: { EmptyView() })
    }
}
