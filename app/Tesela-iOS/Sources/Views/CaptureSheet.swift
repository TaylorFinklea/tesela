import SwiftUI

/// Modal capture composer. Three modes:
///   • **Capture** (default) — text prepends a block to today's daily.
///   • **Palette** — typing `:` switches the sheet into verb mode.
///   • **Recording** — mic button active, live waveform, server-side
///     Whisper transcription on stop.
struct CaptureSheet: View {
    @ObservedObject var mosaic: MockMosaicService
    var seed: String = ""

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss
    @State private var text: String = ""
    @State private var transcribing: Bool = false
    @State private var transcribeError: String? = nil
    @StateObject private var recorder = VoiceRecorder()
    @FocusState private var isFieldFocused: Bool

    private var paletteActive: Bool { text.hasPrefix(":") }
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
    private var isRecording: Bool {
        if case .recording = recorder.state { return true }
        return false
    }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                if paletteActive {
                    paletteChipStrip
                }

                composerField

                if isRecording {
                    recordingPanel
                } else if transcribing {
                    transcribingPanel
                }

                helperRow

                if let err = transcribeError {
                    Text(err)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.typeTask)
                }

                Spacer(minLength: 0)
            }
            .padding(.horizontal, 16)
            .padding(.top, 8)
            .background(theme.bg)
            .navigationTitle(navTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { toolbar }
        }
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
        .onAppear {
            text = seed
            if !isRecording { isFieldFocused = true }
        }
        .onDisappear {
            recorder.cancel()
        }
    }

    private var navTitle: String {
        if isRecording { return "Recording" }
        if transcribing { return "Transcribing…" }
        return paletteActive ? "Palette" : "Capture"
    }

    // ── Composer field ──────────────────────────────────────────────────

    private var composerField: some View {
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
    }

    // ── Recording panel ─────────────────────────────────────────────────

    private var recordingPanel: some View {
        VStack(spacing: 10) {
            waveform
            HStack {
                HStack(spacing: 6) {
                    Circle()
                        .fill(theme.typeTask)
                        .frame(width: 8, height: 8)
                    Text("rec")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.typeTask)
                }
                Text(elapsedLabel)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                Spacer()
                Text(modelLabel)
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
        }
        .padding(12)
        .background(theme.bg2)
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(theme.typeTask.opacity(0.4), lineWidth: 1)
        )
    }

    private var waveform: some View {
        GeometryReader { proxy in
            let bars = 32
            let spacing: CGFloat = 3
            let barW = (proxy.size.width - spacing * CGFloat(bars - 1)) / CGFloat(bars)
            HStack(spacing: spacing) {
                ForEach(0..<bars, id: \.self) { i in
                    // Stagger heights to give the bars some shape — the
                    // most recent bars (right side) reflect current
                    // meter level.
                    let recent = i > bars - 6
                    let base: Float = recent ? recorder.meterLevel : Float.random(in: 0.1 ... 0.35)
                    let height = CGFloat(max(0.08, base)) * proxy.size.height
                    RoundedRectangle(cornerRadius: 1.5)
                        .fill(recent ? theme.accentPrimary : theme.fgFaint.opacity(0.6))
                        .frame(width: barW, height: height)
                        .frame(maxHeight: .infinity)
                }
            }
        }
        .frame(height: 44)
    }

    private var elapsedLabel: String {
        guard case .recording(let elapsed) = recorder.state else { return "00:00" }
        let s = Int(elapsed) % 60
        let m = Int(elapsed) / 60
        return String(format: "%02d:%02d", m, s)
    }

    private var modelLabel: String {
        // Best-effort — the active model is server-managed; iOS knows
        // the most-recent picked id via TranscriptionStore. CaptureSheet
        // doesn't currently take a TranscriptionStore reference, so
        // surface a generic label.
        "whisper · server"
    }

    private var transcribingPanel: some View {
        HStack(spacing: 10) {
            ProgressView()
            Text("Running transcription…")
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(theme.fgMuted)
            Spacer()
        }
        .padding(12)
        .background(theme.bg2)
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    // ── Helper row ──────────────────────────────────────────────────────

    private var helperRow: some View {
        HStack(spacing: 12) {
            Label {
                Text(paletteActive ? "run a verb" : "prepends to today")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            } icon: {
                Image(systemName: paletteActive ? "bolt" : "calendar")
                    .font(.system(size: 11))
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
            Button("Cancel") {
                recorder.cancel()
                dismiss()
            }
            .tint(theme.fgMuted)
        }
        ToolbarItem(placement: .primaryAction) {
            HStack(spacing: 0) {
                Button {
                    Task { await toggleRecording() }
                } label: {
                    Image(systemName: micSymbol)
                        .font(.system(size: 18, weight: .semibold))
                        .foregroundStyle(isRecording ? theme.typeTask : theme.fgMuted)
                        .frame(width: 44, height: 44)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(isRecording ? "Stop recording" : "Voice capture")

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

    private var micSymbol: String {
        switch recorder.state {
        case .recording: return "stop.circle.fill"
        case .requestingPermission: return "mic.slash"
        case .denied: return "mic.slash.fill"
        default: return "mic"
        }
    }

    // MARK: - Recording flow

    private func toggleRecording() async {
        if isRecording {
            await stopAndTranscribe()
        } else {
            transcribeError = nil
            _ = await recorder.start()
            if case .denied = recorder.state {
                transcribeError = "Microphone access denied. Enable in Settings → Tesela."
            }
        }
    }

    private func stopAndTranscribe() async {
        guard let url = recorder.stop() else { return }
        transcribing = true
        defer { transcribing = false }
        do {
            let transcript = try await mosaic.transcribe(audio: url)
            recorder.discardFile()
            let trimmed = transcript.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty {
                transcribeError = "No speech recognized."
            } else {
                // Append to whatever's in the text field so the user
                // can add more, or hit Save to commit immediately.
                if text.isEmpty {
                    text = trimmed
                } else {
                    text += (text.hasSuffix(" ") ? "" : " ") + trimmed
                }
            }
        } catch {
            transcribeError = (error as NSError).localizedDescription
        }
    }

    private func run() {
        if paletteActive {
            dismiss()
        } else {
            mosaic.capture(text)
            dismiss()
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
}
