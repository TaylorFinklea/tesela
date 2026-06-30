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
    /// The composer's text, lifted out of view-local `@State` into a
    /// model AppShell owns — see `CaptureComposer`. This is what makes
    /// an externally-delivered voice transcript reliably land in the
    /// field despite the `tabViewBottomAccessory` recreating this view.
    @ObservedObject var composer: CaptureComposer
    /// `true` for the expanded keyboard-tracking panel, `false` for the
    /// compact bar in the tab accessory. Drives the layout.
    var expanded: Bool = false

    @Environment(\.theme) private var theme

    /// Drives the expanded compose field's first-responder state. A plain
    /// `@State` (not `@FocusState`) because the field is now a `UITextView`-
    /// backed `CaptureTextView` whose focus is driven by an `isFocused` binding.
    @State private var fieldFocused = false

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
        if let manualTarget = composer.manualTarget { return manualTarget }
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
            case .daily, .agenda, .search: return .today
            }
        }
    }

    var body: some View {
        // Bottom-align only the expanded panel, where the multi-line
        // field grows upward and the collapse/send buttons stay anchored.
        // The compact bar is single-line — center it.
        HStack(alignment: expanded ? .bottom : .center, spacing: 8) {
            leadingButton
            targetChip
            typeChip
            composerMiddle
                .frame(maxWidth: .infinity, alignment: .leading)
            trailingButton
        }
        .frame(minHeight: 44)
        .padding(.horizontal, 12)
        .padding(.vertical, expanded ? 6 : 0)
        .padding(.bottom, expanded ? 8 : 0)
        .background {
            // The expanded panel rides above the keyboard via
            // `safeAreaInset` and needs its own solid backing; the
            // compact bar gets its glass from the tab accessory.
            if expanded { theme.bg }
        }
        .overlay(alignment: .top) {
            if expanded {
                Rectangle().fill(theme.lineSoft).frame(height: 1)
            }
        }
    }

    /// The composer's middle slot. Normally the text field; while
    /// recording it becomes a live waveform + timer, during
    /// transcription a spinner, and on failure a tap-to-dismiss error.
    /// Everything stays inside the single accessory row — a row stacked
    /// *above* the composer gets clipped, since `tabViewBottomAccessory`
    /// is a fixed-height pill.
    @ViewBuilder
    private var composerMiddle: some View {
        switch recorder.state {
        case .recording(let elapsed):
            HStack(spacing: 8) {
                VoiceWaveformView(monitor: recorder.levelMonitor)
                Text(elapsedLabel(elapsed))
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                    .foregroundStyle(theme.fgMuted)
                Spacer(minLength: 0)
            }
        case .denied:
            errorChip("Microphone access denied — enable it in Settings.")
        case .failed(let message):
            errorChip("Voice capture failed — \(message)")
        default:
            if recorder.transcribingChunk {
                HStack(spacing: 6) {
                    ProgressView().controlSize(.mini)
                    Text("Transcribing…")
                        .font(.body)
                        .foregroundStyle(theme.fgMuted)
                    Spacer(minLength: 0)
                }
            } else if let error = recorder.transcriptionError {
                errorChip("Couldn't transcribe — \(error)")
            } else if expanded {
                composerField
            } else {
                composerTapTarget
            }
        }
    }

    /// The real editable field — used only in the expanded panel. It
    /// focuses on appear so the keyboard rises and `safeAreaInset`
    /// carries the panel up with it. A `UITextView`-backed `CaptureTextView`
    /// (shared with `GrCaptureSheet`) so the to-be-lifted NLP tokens color live
    /// as the user types; all the prior TextField behaviors are preserved inside
    /// it (draft binding, voice-append, placeholder, autofocus, multi-line
    /// growth) and the keyboard avoidance is untouched (no input accessory).
    private var composerField: some View {
        CaptureTextView(
            text: $composer.draft,
            isFocused: $fieldFocused,
            placeholder: "Capture…",
            textColor: theme.fgDefault,
            tintColor: theme.accentPrimary,
            placeholderColor: theme.fgFaint,
            nlpHighlightRanges: captureHighlightRanges,
            nlpHighlightColor: theme.accentPrimary
        )
        .onAppear {
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(60))
                fieldFocused = true
            }
        }
    }

    /// Inline-NLP highlight spans for the compose field, gated EXACTLY like the
    /// add-time lift (`MockMosaicService.applyCaptureType`): only when a type is
    /// picked, resolving against the live registry but FALLING BACK to the
    /// built-ins when the live registry carries no liftable defs for the picked
    /// type. No type picked → no spans → no coloring.
    private func captureHighlightRanges(_ text: String) -> [NSRange] {
        guard let raw = composer.manualTag?.trimmingCharacters(in: .whitespaces),
              !raw.isEmpty else { return [] }
        let tagToken = raw.hasPrefix("#") ? raw : "#\(raw)"
        let canonical = String(tagToken.dropFirst())
        let live = mosaic.propertyRegistry
        let reg = live.hasLiftableDefs(forTag: canonical) ? live : PropertyRegistry.buildBuiltins()
        return InlineNLP.detectHighlightRanges(in: text, tags: [tagToken], registry: reg)
    }

    /// Compact-bar middle: a tap target showing the draft (or the
    /// placeholder). Tapping expands the composer — the compact bar
    /// deliberately never focuses a field itself, which would drop the
    /// keyboard behind the tab accessory.
    private var composerTapTarget: some View {
        Button {
            withAnimation(.snappy(duration: 0.28)) {
                composer.isExpanded = true
            }
        } label: {
            Text(composer.draft.isEmpty ? "Capture…" : composer.draft)
                .font(.body)
                .foregroundStyle(composer.draft.isEmpty ? theme.fgFaint : theme.fgDefault)
                .lineLimit(composer.draft.isEmpty ? 1 : 4)
                .multilineTextAlignment(.leading)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    /// A surfaced voice error — tap anywhere on it to dismiss and
    /// return the composer to the text field.
    private func errorChip(_ text: String) -> some View {
        Button {
            recorder.dismissError()
        } label: {
            Text(text)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(theme.typeTask)
                .lineLimit(2)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func elapsedLabel(_ elapsed: TimeInterval) -> String {
        let total = Int(elapsed)
        return String(format: "%d:%02d", total / 60, total % 60)
    }

    /// Leading slot: a collapse chevron in the expanded panel, the `+`
    /// attach button in the compact bar.
    @ViewBuilder
    private var leadingButton: some View {
        if expanded {
            Button {
                withAnimation(.snappy(duration: 0.28)) {
                    composer.isExpanded = false
                }
                fieldFocused = false
            } label: {
                Image(systemName: "chevron.down")
                    .font(.system(size: 17, weight: .semibold))
                    .foregroundStyle(theme.fgMuted)
                    .frame(width: 30, height: 30)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Collapse composer")
        } else {
            plusButton
        }
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
                composer.manualTarget = .today
            } label: {
                Label("Today", systemImage: "calendar")
            }
            Button {
                composer.manualTarget = .inbox
            } label: {
                Label("Inbox", systemImage: "tray")
            }
            if let page = context.currentPage {
                Button {
                    composer.manualTarget = .page(slug: page.slug, title: page.title)
                } label: {
                    Label("Add to \(page.title)", systemImage: "doc.text")
                }
            }
            if let block = context.focusedBlock {
                Button {
                    composer.manualTarget = .childOf(
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

    /// Type/tag picker chip. Tap → menu of "Note" (plain, no NLP) + the
    /// registry's types (Task, Project, …). Picking a type tags the block
    /// and runs inline NLP at add-time. Icon-only at rest; shows the chosen
    /// type's name (tinted) when active — mirrors `targetChip`'s styling.
    private var typeChip: some View {
        let active = composer.manualTag != nil
        return Menu {
            Button {
                composer.manualTag = nil
            } label: {
                Label("Note", systemImage: "text.alignleft")
            }
            ForEach(captureTypeNames(), id: \.self) { type in
                Button {
                    composer.manualTag = type
                } label: {
                    Label(type, systemImage: "number")
                }
            }
        } label: {
            Group {
                if active {
                    Text(composer.manualTag ?? "")
                        .font(.system(size: 12, weight: .semibold))
                        .lineLimit(1)
                        .padding(.horizontal, 9)
                } else {
                    Image(systemName: "number")
                        .font(.system(size: 14, weight: .semibold))
                        .frame(width: 30)
                }
            }
            .frame(height: 30)
            .foregroundStyle(active ? theme.typeTask : theme.fgMuted)
            .background(Capsule().fill(active ? theme.typeTask.opacity(0.16) : theme.bg3))
            .contentShape(Capsule())
        }
        .accessibilityLabel("Capture type: \(composer.manualTag ?? "Note")")
    }

    /// The registry's type names with a built-in fallback (Task/Project)
    /// for a not-yet-synced registry, so the picker is never empty.
    private func captureTypeNames() -> [String] {
        let names = mosaic.propertyRegistry.typeNames()
        return names.isEmpty ? PropertyRegistry.buildBuiltins().typeNames() : names
    }

    /// Mic when draft is empty, send arrow when there's text. Mic
    /// toggles streaming voice capture; transcript appends into the
    /// draft so the user can review before submitting.
    @ViewBuilder
    private var trailingButton: some View {
        // Recording always shows Stop, even with draft text present.
        if isRecording || composer.draft.trimmingCharacters(in: .whitespaces).isEmpty {
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
        let trimmed = composer.draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        mosaic.capture(trimmed, target: resolvedTarget, tag: composer.manualTag)
        // Clear a child-block target after submit so the next capture
        // doesn't insert under the same (possibly stale) parent.
        if case .childOf = resolvedTarget {
            composer.manualTarget = nil
        }
        // Reset the type so the next capture defaults to a plain note.
        composer.manualTag = nil
        composer.draft = ""
        fieldFocused = false
        withAnimation(.snappy(duration: 0.28)) {
            composer.isExpanded = false
        }
    }

    private func toggleRecording() async {
        if isRecording {
            recorder.stop()
            return
        }
        // The finished transcript arrives via `recorder.lastTranscript`,
        // which AppShell observes and feeds into `composer` — not a
        // callback or a `.onChange` on this view, both of which can miss
        // the live instance once the accessory recreates the bar.
        _ = await recorder.start(using: engine)
    }
}

/// The capture composer's text, lifted out of `CaptureBar`'s view-local
/// `@State` into a reference type. `CaptureBar` lives in
/// `tabViewBottomAccessory`, which recreates / re-identifies its content
/// aggressively — so `@State` there is an unreliable home for text that
/// is written from outside (a finished voice transcript). AppShell owns
/// one of these, feeds transcripts in, and passes it to the bar.
@MainActor
final class CaptureComposer: ObservableObject {
    @Published var draft: String = ""
    /// True while the composer is expanded into the tall, keyboard-
    /// tracking panel rather than the compact bar in the tab accessory.
    @Published var isExpanded: Bool = false
    /// Manually-chosen capture target (from the chip menu); `nil` means
    /// resolve from settings + active tab + page context. Held here so
    /// the compact bar and the expanded panel always agree.
    @Published var manualTarget: CaptureTarget? = nil
    /// Manually-chosen TYPE for the capture (from the type picker), e.g.
    /// "Task". `nil` means a plain `.note` block with no tag and no NLP —
    /// today's behavior. When set, the captured block is tagged `#<type>`
    /// and run through `InlineNLP.detectLifts` at add-time, so "Test p1
    /// tomorrow" with type=Task lands as text "Test" + Priority p1 +
    /// Deadline tomorrow. Held here so the compact bar and the expanded
    /// sheet agree; reset on submit so a type is a per-capture choice, not
    /// a sticky mode.
    @Published var manualTag: String? = nil
    /// Set by the compact bar's mic button just before expanding: the
    /// sheet opens straight into voice mode — start recording, and do
    /// NOT autofocus the text field (the keyboard rising mid-present
    /// mangled the sheet layout). Consumed (reset) by the sheet's
    /// onAppear.
    @Published var pendingVoiceCapture: Bool = false

    /// Append dictated / transcribed text, separated by a space.
    func append(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        if draft.isEmpty {
            draft = trimmed
        } else {
            draft += (draft.hasSuffix(" ") ? "" : " ") + trimmed
        }
        voiceDiag("composer: draft now \(draft.count) chars")
    }
}

/// Opt the EXPANDED capture panel out of SwiftUI's automatic (predictive-bar-
/// blind) keyboard avoidance so its manual `keyboard.overlap` padding is the
/// sole lift — preventing a double-lift. Inert for the compact accessory bar.
private struct ExpandedKeyboardLift: ViewModifier {
    let active: Bool
    let overlap: CGFloat
    func body(content: Content) -> some View {
        if active {
            content
                .ignoresSafeArea(.keyboard, edges: .bottom)
                .animation(.easeOut(duration: 0.22), value: overlap)
        } else {
            content
        }
    }
}

/// Live microphone waveform shown in the capture bar while recording —
/// a row of bars, each a recent RMS sample, so the user can see their
/// voice is being picked up. Observes `AudioLevelMonitor` directly so
/// only this small view re-renders at the audio-buffer rate.
struct VoiceWaveformView: View {
    @ObservedObject var monitor: AudioLevelMonitor
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 2.5) {
            ForEach(Array(monitor.levels.enumerated()), id: \.offset) { _, level in
                Capsule()
                    .fill(theme.accentPrimary)
                    .frame(width: 2.5, height: 3 + CGFloat(level) * 19)
            }
        }
        .frame(height: 22)
        .animation(.linear(duration: 0.08), value: monitor.levels)
    }
}
