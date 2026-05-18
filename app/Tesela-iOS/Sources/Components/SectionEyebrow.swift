import SwiftUI

/// Mono-uppercase section eyebrow used above grouped lists.
/// Mirrors the web's `.mono` uppercase header style.
struct SectionEyebrow: View {
    let title: String
    var hint: String? = nil

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title.uppercased())
                .font(.system(size: 10, design: .monospaced))
                .tracking(1.2)
                .foregroundStyle(theme.fgFaint)
            if let hint {
                Spacer(minLength: 8)
                Text(hint)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
        }
        .padding(.horizontal, 18)
        .padding(.top, 16)
        .padding(.bottom, 6)
    }
}
