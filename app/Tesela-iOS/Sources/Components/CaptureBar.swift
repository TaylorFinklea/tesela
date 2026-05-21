import SwiftUI

/// Persistent capture bar shown in `tabViewBottomAccessory` — Slack's
/// composer pattern adapted to iOS 26 Liquid Glass. Layout:
///
///   [+ attach]  [target icon]  [text field]  [mic | send]
///
/// `+` and the target icon are at the leading edge so the text field
/// grows naturally toward the trailing edge as the user types. The
/// target chip is icon-only at rest; tapping it opens a menu with full
/// labels — Today, Inbox, the current page (when Library has one open),
/// and "Add as child" when a block is focused.
struct CaptureBar: View {
    @ObservedObject var mosaic: MockMosaicService
    /// Active tab — drives the context-aware default when the user
    /// hasn't picked a manual target.
    let activeTab: AppTab
    /// On-device transcription store. Optional because Library/Inbox
    /// surfaces don't always need to wire one through.
    var transcription: TranscriptionStore? = nil
    /// Ambient capture context (current page, focused block). Set by
    /// other views; the bar reads it to populate its menu.
    var context: CaptureContext
    /// Voice recorder lifted from `@StateObject` to a parent-owned
    /// instance so it survives this bar being added/removed from the
    /// `tabViewBottomAccessory` slot — `AVAudioEngine` init is heavy
    /// enough to cause Fence Hangs when paid repeatedly.
    @ObservedObject var recorder: StreamingVoiceRecorder

    @Environment(\.theme) private var theme
    @ObservedObject private var diag = VoiceDiagnostics.shared

    @State private var draft: String = ""
    @State private var manualTarget: CaptureTarget? = nil
    @State private var transcribeError: String? = nil
    @FocusState private var fieldFocused: Bool

    @AppStorage("captureDefaultTarget") private var captureDefault: CaptureDefault = .contextAware
    @AppStorage("voice.useOnDevice") private var useOnDevice: Bool = true

    private var engine: TranscriptionEngine {
        if useOnDevice, let transcription {
            return LocalTranscriptionEngine(store: transcription)
        }
        return ServerTranscriptionEngine(mosaic: mosaic)
    }

    private var isRecording: Bool {
        if case .recording = recorder.state { return true }
        return false
    }

    /// Where the next submit will land. Manual chip selection wins;
    /// otherwise resolve from settings + active tab + page context.
    private var resolvedTarget: CaptureTarget {
        if let manualTarget { return manualTarget }
        switch captureDefault {
        case .alwaysToday:    return .today
        case .alwaysInbox:    return .inbox
        case .contextAware:
            switch activeTab {
            case .inbox:                return .inbox
            case .library:
                if let page = context.currentPage {
                    return .page(slug: page.slug, title: page.title)
                }
                return .today
            case .daily, .search:       return .today
            }
        }
    }

    var body: some View {
        VStack(spacing: 2) {
            statusLine
            HStack(spacing: 8) {
                plusButton
                targetChip
                TextField("Capture…", text: $draft, axis: .vertical)
                    .focused($fieldFocused)
                    .submitLabel(.send)
                    .onSubmit(submit)
                    .lineLimit(1...4)
                    .font(.body)
                    .foregroundStyle(theme.fgDefault)
                    .tint(theme.accentPrimary)
                trailingButton
            }
            .frame(minHeight: 44)
        }
        .padding(.horizontal, 12)
    }

    /// One-line voice feedback above the composer: a transcription
    /// error, a recorder failure, or a "transcribing…" indicator.
    /// Without this every voice failure is silent — the bar just looks
    /// like it does nothing.
    @ViewBuilder
    private var statusLine: some View {
        if let status = voiceStatus {
            Text(status.text)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(status.isError ? theme.typeTask : theme.fgMuted)
                .lineLimit(2)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var voiceStatus: (text: String, isError: Bool)? {
        if let transcribeError {
            return ("Transcription failed — \(transcribeError)", true)
        }
        switch recorder.state {
        case .denied:
            return ("Microphone access denied — enable it in Settings.", true)
        case .failed(let message):
            return ("Voice capture failed — \(message)", true)
        default:
            break
        }
        // The live voice-path diagnostic — current phase while working,
        // final outcome (e.g. "0 chars transcribed") after. Empty until
        // the first recording of the session.
        if !diag.lastLine.isEmpty {
            return ("Voice: \(diag.lastLine)", false)
        }
        return nil
    }

    /// Leftmost `+` for future attachment support. Stub for now.
    private var plusButton: some View {
        Button {
            // Attachments stub — not wired yet.
        } label: {
            Image(systemName: "plus")
                .font(.system(size: 20, weight: .regular))
                .foregroundStyle(theme.fgMuted)
                .frame(width: 30, height: 30)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Attach")
    }

    /// Icon-only target chip. Tap → menu with full labels. Menu items
    /// are context-aware: "this page" appears when Library has a page
    /// open; "Add as child" appears when a block is focused.
    private var targetChip: some View {
        Menu {
            Button {
                manualTarget = .today
            } label: {
                Label("Today", systemImage: "calendar")
            }
            Button {
                manualTarget = .inbox
            } label: {
                Label("Inbox", systemImage: "tray")
            }
            if let page = context.currentPage {
                Button {
                    manualTarget = .page(slug: page.slug, title: page.title)
                } label: {
                    Label("Add to \(page.title)", systemImage: "doc.text")
                }
            }
            if let block = context.focusedBlock {
                Button {
                    manualTarget = .childOf(
                        parentId: block.id,
                        parentPreview: block.preview,
                        pageSlug: block.pageSlug
                    )
                } label: {
                    Label(childLabel(block), systemImage: "arrow.turn.down.right")
                }
            }
        } label: {
            Image(systemName: resolvedTarget.systemImage)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(theme.fgMuted)
                .frame(width: 30, height: 30)
                .background(Capsule().fill(theme.bg3))
                .contentShape(Capsule())
        }
        .accessibilityLabel("Capture target: \(resolvedTarget.label)")
    }

    /// Mic when draft is empty, send arrow when there's text. Mic
    /// toggles streaming voice capture; transcript appends into the
    /// draft so the user can review before submitting.
    @ViewBuilder
    private var trailingButton: some View {
        if draft.trimmingCharacters(in: .whitespaces).isEmpty {
            Button {
                Task { await toggleRecording() }
            } label: {
                Image(systemName: isRecording ? "stop.circle.fill" : "mic")
                    .font(.system(size: isRecording ? 24 : 18, weight: .regular))
                    .foregroundStyle(isRecording ? theme.accentPrimary : theme.fgMuted)
                    .frame(width: 30, height: 30)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(isRecording ? "Stop recording" : "Record voice note")
        } else {
            Button(action: submit) {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.system(size: 26, weight: .regular))
                    .foregroundStyle(theme.accentPrimary)
                    .frame(width: 30, height: 30)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Send capture")
        }
    }

    private func childLabel(_ block: CaptureBlockRef) -> String {
        let trimmed = block.preview.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return "Add as child" }
        let preview = trimmed.count <= 24 ? trimmed : String(trimmed.prefix(22)) + "…"
        return "Add as child of \u{201C}\(preview)\u{201D}"
    }

    private func submit() {
        let trimmed = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        mosaic.capture(trimmed, target: resolvedTarget)
        draft = ""
        fieldFocused = false
        // Clear manual target after a child-block submit so the next
        // capture doesn't try to insert under the same (possibly
        // stale) parent.
        if case .childOf = resolvedTarget {
            manualTarget = nil
        }
    }

    private func toggleRecording() async {
        if isRecording {
            recorder.stop()
            return
        }
        transcribeError = nil
        recorder.onChunk = { transcript in
            appendTranscript(transcript)
        }
        recorder.onError = { msg in
            transcribeError = msg
        }
        _ = await recorder.start(using: engine)
    }

    private func appendTranscript(_ transcript: String) {
        let trimmed = transcript.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        if draft.isEmpty {
            draft = trimmed
        } else {
            draft += (draft.hasSuffix(" ") ? "" : " ") + trimmed
        }
    }
}
