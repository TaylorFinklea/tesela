import SwiftUI

/// Modal capture composer presented from the bottom-accessory pill.
/// Two modes:
///   • **Capture** (default) — text prepends a block to today's daily.
///   • **Palette** — typing `:` switches the sheet into verb mode;
///     matching verbs from `mosaic.palette` appear as chip rows above the
///     composer, the Save button becomes "Run" and dispatches the verb.
///
/// Per decision #6 (`.docs/designs/2026-05-18-ios-design-followup.md`).
struct CaptureSheet: View {
    @ObservedObject var mosaic: MockMosaicService
    var seed: String = ""

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss
    @State private var text: String = ""
    @State private var isRecording: Bool = false
    @FocusState private var isFieldFocused: Bool

    /// Palette mode active iff the first character is `:`.
    private var paletteActive: Bool { text.hasPrefix(":") }

    /// Verb filter — the text after `:`.
    private var paletteFilter: String {
        guard text.hasPrefix(":") else { return "" }
        return String(text.dropFirst()).lowercased()
    }

    private var matchingVerbs: [PaletteVerb] {
        guard paletteActive else { return [] }
        let f = paletteFilter
        return mosaic.palette.filter {
            f.isEmpty || $0.name.dropFirst().lowercased().hasPrefix(f)
        }
    }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                if paletteActive {
                    paletteChipStrip
                }

                TextField(
                    paletteActive ? "verb command…" : "capture to today…",
                    text: $text,
                    axis: .vertical
                )
                .font(.system(size: 17, design: paletteActive ? .monospaced : .default))
                .foregroundStyle(theme.fgDefault)
                .tint(theme.accentPrimary)
                .focused($isFieldFocused)
                .lineLimit(3 ... 8)
                .padding(14)
                .background(theme.bg2)
                .clipShape(RoundedRectangle(cornerRadius: 12))
                .overlay(
                    RoundedRectangle(cornerRadius: 12)
                        .stroke(
                            paletteActive ? theme.accentPrimary.opacity(0.6) : theme.line,
                            lineWidth: 1
                        )
                )

                helperRow
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 16)
            .padding(.top, 8)
            .background(theme.bg)
            .navigationTitle(paletteActive ? "Palette" : "Capture")
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

    // ── Palette chip strip ──────────────────────────────────────────────

    private var paletteChipStrip: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(matchingVerbs) { verb in
                    Button {
                        text = verb.name
                    } label: {
                        VStack(alignment: .leading, spacing: 1) {
                            Text(verb.name)
                                .font(.system(size: 11.5, weight: .semibold, design: .monospaced))
                                .foregroundStyle(theme.accentPrimary)
                            Text(verb.hint)
                                .font(.system(size: 10, design: .monospaced))
                                .foregroundStyle(theme.fgFaint)
                                .lineLimit(1)
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 6)
                        .frame(minWidth: 140, maxWidth: 220, alignment: .leading)
                        .background(theme.bg3)
                        .overlay(
                            RoundedRectangle(cornerRadius: 6)
                                .stroke(theme.line, lineWidth: 1)
                        )
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                    }
                    .buttonStyle(.plain)
                }
                if matchingVerbs.isEmpty {
                    Text("no matching verbs")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 6)
                }
            }
        }
        .scrollClipDisabled()
    }

    // ── Helper row beneath the composer ─────────────────────────────────

    private var helperRow: some View {
        HStack(spacing: 12) {
            Label {
                Text(paletteActive ? "run a verb" : "prepends to today")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            } icon: {
                Icon(name: paletteActive ? .bolt : .daily, size: 14)
                    .foregroundStyle(theme.fgFaint)
            }
            Spacer()
            Text("\(text.count) chars")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
        }
    }

    // ── Toolbar — Cancel · Mic · Save / Run ─────────────────────────────

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
                    run()
                } label: {
                    Text(paletteActive ? "Run" : "Save")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(text.isEmpty ? theme.fgFaint : theme.accentPrimary)
                }
                .disabled(text.isEmpty || (paletteActive && matchingVerbs.isEmpty))
            }
        }
    }

    private func run() {
        if paletteActive {
            // Verb dispatch is a stub — real implementations land in
            // Phase 15 alongside the FFI surface. For now we close
            // the sheet so the user sees the action terminated.
            dismiss()
        } else {
            mosaic.capture(text)
            dismiss()
        }
    }
}
