import SwiftUI

/// Persistent capture bar pinned above the bottom tab bar. Default
/// behavior captures to today's daily; later phases extend this with
/// palette mode (`:` prefix) per decision #6.
struct CaptureBar: View {
    @Binding var text: String
    var placeholder: String = "capture to today…"
    var onSend: () -> Void = {}
    var onMic: () -> Void = {}

    @Environment(\.theme) private var theme
    @FocusState private var isFocused: Bool

    var body: some View {
        HStack(spacing: 10) {
            // Round-square + button. Tapping it focuses the text field
            // (most common path) or sends if there's already text.
            Button {
                if text.isEmpty {
                    isFocused = true
                } else {
                    onSend()
                }
            } label: {
                Icon(name: .plus, size: 18)
                    .foregroundStyle(theme.bg)
                    .frame(width: 30, height: 30)
                    .background(theme.accentPrimary)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            .buttonStyle(.plain)

            ZStack(alignment: .leading) {
                if text.isEmpty {
                    Text(placeholder)
                        .italic()
                        .font(.system(size: 14))
                        .foregroundStyle(theme.fgFaint)
                }
                TextField("", text: $text)
                    .font(.system(size: 15))
                    .foregroundStyle(theme.fgDefault)
                    .focused($isFocused)
                    .submitLabel(.send)
                    .onSubmit(onSend)
                    .tint(theme.accentPrimary)
            }

            Button(action: onMic) {
                Icon(name: .mic, size: 20)
                    .foregroundStyle(theme.fgSubtle)
                    .frame(width: 36, height: 36)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Voice capture")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(theme.bg2)
        .overlay(alignment: .top) {
            Rectangle()
                .fill(theme.line)
                .frame(height: 1)
        }
    }
}
