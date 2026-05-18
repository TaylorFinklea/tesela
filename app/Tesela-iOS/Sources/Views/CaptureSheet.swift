import SwiftUI

/// Modal capture composer presented from the bottom-accessory capture
/// trigger. Holds the text field, microphone, and Save action. Replaces
/// the always-on `CaptureBar` chrome with a native iOS 26 sheet.
///
/// Voice (Parakeet v3) recording UI lives here too in a later phase —
/// for now the mic button is a no-op placeholder.
struct CaptureSheet: View {
    @ObservedObject var mosaic: MockMosaicService
    /// Optional seed text — used when the user already typed something
    /// in the bottom accessory pill (Phase 7 palette-mode prep).
    var seed: String = ""

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss
    @State private var text: String = ""
    @State private var isRecording: Bool = false
    @FocusState private var isFieldFocused: Bool

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                // Composer
                TextField("capture to today…", text: $text, axis: .vertical)
                    .font(.system(size: 17))
                    .foregroundStyle(theme.fgDefault)
                    .tint(theme.accentPrimary)
                    .focused($isFieldFocused)
                    .lineLimit(3 ... 8)
                    .padding(14)
                    .background(theme.bg2)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(theme.line, lineWidth: 1)
                    )

                helperRow

                Spacer(minLength: 0)
            }
            .padding(.horizontal, 16)
            .padding(.top, 8)
            .background(theme.bg)
            .navigationTitle("Capture")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { toolbar }
        }
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
        .onAppear {
            text = seed
            isFieldFocused = true
        }
    }

    // ── Helper row beneath the composer ─────────────────────────────────

    private var helperRow: some View {
        HStack(spacing: 12) {
            Label {
                Text("prepends to today")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            } icon: {
                Icon(name: .daily, size: 14)
                    .foregroundStyle(theme.fgFaint)
            }
            Spacer()
            Text("\(text.count) chars")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
        }
    }

    // ── Toolbar — Cancel · Mic · Save ───────────────────────────────────

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        ToolbarItem(placement: .cancellationAction) {
            Button("Cancel") { dismiss() }
                .tint(theme.fgMuted)
        }
        ToolbarItem(placement: .primaryAction) {
            HStack(spacing: 0) {
                Button {
                    isRecording.toggle()
                } label: {
                    Icon(name: .mic, size: 20)
                        .foregroundStyle(isRecording ? theme.typeTask : theme.fgMuted)
                        .frame(width: 44, height: 44)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Voice capture")

                Button {
                    send()
                } label: {
                    Text("Save")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(text.isEmpty ? theme.fgFaint : theme.accentPrimary)
                }
                .disabled(text.isEmpty)
            }
        }
    }

    private func send() {
        mosaic.capture(text)
        dismiss()
    }
}
