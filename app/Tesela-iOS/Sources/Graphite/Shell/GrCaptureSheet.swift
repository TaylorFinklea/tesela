import SwiftUI

/// Graphite capture chrome — mirrors `Sources/Components/CaptureBar.swift`
/// (the pill → sheet structure, the `capture(_:target:)` call, the
/// `manualTarget` / `draft` / `isExpanded` state) in the Graphite mobile
/// presentation (`.docs/ai/design/graphite/mobile/grm-shell.jsx` —
/// `.grm-capbar`, `.grm-sheet`, `.grm-sheetdim`).
///
/// Behavior is 100% reused: the same `CaptureComposer` (owned by the
/// shell), the same `MockMosaicService.capture(_:target:)`, the same
/// `StreamingVoiceRecorder`, and the same target-resolution logic as
/// `CaptureBar`. Only the presentation is new.

// MARK: - Target resolution (mirrors CaptureBar.resolvedTarget / submit)

/// Resolve where the next submit lands. Mirrors `CaptureBar.resolvedTarget`:
/// a manually-picked target wins; otherwise the context-aware default
/// derived from the active tab + ambient page context.
@MainActor
private func grResolvedTarget(
    composer: CaptureComposer,
    activeTab: AppTab,
    captureDefault: CaptureDefault,
    context: CaptureContext
) -> CaptureTarget {
    if let manualTarget = composer.manualTarget { return manualTarget }
    switch captureDefault {
    case .alwaysToday: return .today
    case .alwaysInbox: return .inbox
    case .contextAware:
        switch activeTab {
        case .inbox: return .inbox
        case .library:
            if let page = context.currentPage {
                return .page(slug: page.slug, title: page.title)
            }
            return .today
        case .daily, .agenda, .search: return .today
        }
    }
}

/// The capture-target menu items (Today / Inbox / current page / child of
/// the focused block), shared by the compact bar's swatch and the expanded
/// sheet's chooser so they never drift. Each writes `composer.manualTarget`.
@MainActor
@ViewBuilder
private func targetMenuItems(composer: CaptureComposer, context: CaptureContext) -> some View {
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
            Label("Add as child", systemImage: "arrow.turn.down.right")
        }
    }
}

// MARK: - Compact capture bar (tabViewBottomAccessory)

/// The compact Graphite capture pill shown in `tabViewBottomAccessory`.
/// Mirrors `.grm-capbar`: `[+]  [target]  Capture…  [mic]`. Tapping the
/// text slot or `+` expands the composer into `GrCaptureSheet` — exactly
/// like `CaptureBar`'s compact bar, which never focuses a field itself
/// (that would drop the keyboard behind the accessory).
struct GrCaptureBar: View {
    @ObservedObject var mosaic: MockMosaicService
    let activeTab: AppTab
    var transcription: TranscriptionStore? = nil
    var context: CaptureContext
    @ObservedObject var recorder: StreamingVoiceRecorder
    @ObservedObject var composer: CaptureComposer

    @Environment(\.theme) private var theme

    @AppStorage("captureDefaultTarget") private var captureDefault: CaptureDefault = .contextAware

    private var resolvedTarget: CaptureTarget {
        grResolvedTarget(
            composer: composer,
            activeTab: activeTab,
            captureDefault: captureDefault,
            context: context
        )
    }

    private func expand() {
        withAnimation(.snappy(duration: 0.28)) { composer.isExpanded = true }
    }

    var body: some View {
        HStack(spacing: 11) {
            Button(action: expand) {
                GrIcon(name: "plus", size: 21)
                    .foregroundStyle(theme.fgMuted)
                    .frame(width: 30, height: 30)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Attach")

            // Target swatch — a tap-to-choose target picker (Today / Inbox /
            // current page / child-of-focused), mirroring the expanded
            // sheet's `targetMenu`. It used to be a dead `Image` that only
            // *displayed* the target; the tinted swatch looked tappable but
            // did nothing.
            Menu {
                targetMenuItems(composer: composer, context: context)
            } label: {
                Image(systemName: resolvedTarget.systemImage)
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(theme.typeNote)
                    .frame(width: 34, height: 34)
                    .background(
                        RoundedRectangle(cornerRadius: 11)
                            .fill(theme.accentSecondary.opacity(0.18))
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 11)
                            .stroke(theme.accentSecondary.opacity(0.26), lineWidth: 1)
                    )
            }
            .accessibilityLabel("Capture target: \(resolvedTarget.label)")

            Button(action: expand) {
                Text(composer.draft.isEmpty ? "Capture…" : composer.draft)
                    .font(.system(size: 16))
                    .foregroundStyle(composer.draft.isEmpty ? theme.fgSubtle : theme.fgDefault)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            Button {
                // Voice-first expand: flag the composer so the sheet opens
                // recording with NO text-field autofocus — the keyboard
                // rising while the sheet presented left it half-behind the
                // keyboard with the mic blob clipped (2026-06-10 product
                // test).
                composer.pendingVoiceCapture = true
                expand()
            } label: {
                GrIcon(name: "microphone", size: 21)
                    .foregroundStyle(theme.fgMuted)
                    .frame(width: 38, height: 38)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Record voice note")
        }
        .frame(minHeight: 44)
        .padding(.horizontal, 8)
    }
}

// MARK: - Expanded capture sheet (.grm-sheet)

/// The expanded Graphite capture sheet. Mirrors `.grm-sheet` /
/// `.grm-sheetdim`: a dimmer over the canvas + a bottom card with a grab
/// handle, a head row (title + target chooser), the compose field, and a
/// footer (record button + send). Bound to the same `CaptureComposer`
/// and `MosaicService.capture(_:target:)` as `CaptureBar`; the field
/// rides above the keyboard via the shell's `safeAreaInset(.bottom)`.
struct GrCaptureSheet: View {
    @ObservedObject var mosaic: MockMosaicService
    let activeTab: AppTab
    var transcription: TranscriptionStore? = nil
    var context: CaptureContext
    @ObservedObject var recorder: StreamingVoiceRecorder
    @ObservedObject var composer: CaptureComposer

    @Environment(\.theme) private var theme

    @FocusState private var fieldFocused: Bool

    /// Live vertical drag of the swipe-to-dismiss gesture. Positive =
    /// dragging down; the sheet follows the finger and either dismisses
    /// past the threshold or springs back.
    @State private var dragOffset: CGFloat = 0

    @AppStorage("captureDefaultTarget") private var captureDefault: CaptureDefault = .contextAware
    @AppStorage("voice.useOnDevice") private var useOnDevice: Bool = true

    /// Same engine selection as `CaptureBar`: on-device when enabled and
    /// a store is wired, else the server engine.
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

    private var resolvedTarget: CaptureTarget {
        grResolvedTarget(
            composer: composer,
            activeTab: activeTab,
            captureDefault: captureDefault,
            context: context
        )
    }

    var body: some View {
        VStack(spacing: 0) {
            grabHandle
            head
            composeField
            footer
        }
        .padding(.horizontal, 18)
        .padding(.top, 10)
        .background(theme.bg2)
        .overlay(alignment: .top) {
            Rectangle().fill(theme.line).frame(height: 1)
        }
        .clipShape(.rect(topLeadingRadius: 26, topTrailingRadius: 26))
        .shadow(color: .black.opacity(0.5), radius: 25, y: -16)
        // Swipe-down dismissal — the grabber promised it, but only the
        // chevron button collapsed the sheet. Attached with `.gesture`
        // so child interactions (the TextField's own drag/selection, the
        // buttons) keep priority; drags on the handle / head / footer /
        // padding reach us.
        .offset(y: max(0, dragOffset))
        .gesture(dismissDrag)
        .onAppear {
            if composer.pendingVoiceCapture {
                // Voice-first open (compact bar's mic): start recording
                // immediately and skip the text-field autofocus so the
                // keyboard can't fight the presentation.
                composer.pendingVoiceCapture = false
                Task { await toggleRecording() }
            } else {
                Task { @MainActor in
                    try? await Task.sleep(for: .milliseconds(60))
                    fieldFocused = true
                }
            }
        }
    }

    /// Drag-to-dismiss: track the finger while dragging down, collapse
    /// when released past the distance threshold (or flicked), spring
    /// back otherwise.
    private var dismissDrag: some Gesture {
        DragGesture(minimumDistance: 12)
            .onChanged { value in
                dragOffset = value.translation.height
            }
            .onEnded { value in
                let flick = value.predictedEndTranslation.height > 160
                if value.translation.height > 80 || flick {
                    fieldFocused = false
                    withAnimation(.snappy(duration: 0.28)) {
                        composer.isExpanded = false
                    }
                    dragOffset = 0
                } else {
                    withAnimation(.snappy(duration: 0.2)) { dragOffset = 0 }
                }
            }
    }

    private var grabHandle: some View {
        RoundedRectangle(cornerRadius: 3)
            .fill(theme.fgFaint.opacity(0.6))
            .frame(width: 38, height: 5)
            .padding(.bottom, 12)
    }

    private var head: some View {
        HStack(spacing: 10) {
            Text("Capture")
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Spacer(minLength: 0)
            targetMenu
        }
        .padding(.bottom, 14)
    }

    /// Native input chooser — the target menu. Mirrors `CaptureBar`'s
    /// `targetChip` exactly (same menu items, same `manualTarget` writes).
    private var targetMenu: some View {
        Menu {
            targetMenuItems(composer: composer, context: context)
        } label: {
            HStack(spacing: 6) {
                Text("to")
                    .foregroundStyle(theme.fgFaint)
                Image(systemName: resolvedTarget.systemImage)
                    .font(.system(size: 11))
                Text(resolvedTarget.label)
                    .lineLimit(1)
            }
            .font(.system(size: 11, design: .monospaced))
            .foregroundStyle(theme.fgSubtle)
        }
        .accessibilityLabel("Capture target: \(resolvedTarget.label)")
    }

    @ViewBuilder
    private var composeField: some View {
        switch recorder.state {
        case .recording(let elapsed):
            HStack(spacing: 8) {
                VoiceWaveformView(monitor: recorder.levelMonitor)
                Text(elapsedLabel(elapsed))
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                    .foregroundStyle(theme.fgMuted)
                Spacer(minLength: 0)
            }
            .frame(minHeight: 54, alignment: .top)
        case .denied:
            errorChip("Microphone access denied — enable it in Settings.")
        case .failed(let message):
            errorChip("Voice capture failed — \(message)")
        default:
            if recorder.transcribingChunk {
                HStack(spacing: 6) {
                    ProgressView().controlSize(.mini)
                    Text("Transcribing…")
                        .font(.system(size: 16))
                        .foregroundStyle(theme.fgMuted)
                    Spacer(minLength: 0)
                }
                .frame(minHeight: 54, alignment: .top)
            } else if let error = recorder.transcriptionError {
                errorChip("Couldn't transcribe — \(error)")
            } else {
                TextField("Capture…", text: $composer.draft, axis: .vertical)
                    .focused($fieldFocused)
                    .submitLabel(.send)
                    .onSubmit(submit)
                    .lineLimit(1...12)
                    .font(.system(size: 16))
                    .foregroundStyle(theme.fgDefault)
                    .tint(theme.accentPrimary)
                    .frame(minHeight: 54, alignment: .top)
            }
        }
    }

    private var footer: some View {
        HStack(spacing: 10) {
            // Record / stop — the coral disc per `.grm-recbtn`.
            Button {
                Task { await toggleRecording() }
            } label: {
                Image(systemName: isRecording ? "stop.fill" : "mic.fill")
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(Color(hex: 0x10110F))
                    .frame(width: 50, height: 50)
                    .background(Circle().fill(theme.accentPrimary))
                    .overlay(
                        Circle().stroke(theme.accentPrimary.opacity(0.15), lineWidth: 6)
                    )
            }
            .buttonStyle(.plain)
            .accessibilityLabel(isRecording ? "Stop recording" : "Record voice note")

            Spacer(minLength: 0)

            // Collapse — return to the compact bar without sending.
            Button {
                withAnimation(.snappy(duration: 0.28)) { composer.isExpanded = false }
                fieldFocused = false
            } label: {
                GrIcon(name: "chevron-down", size: 17)
                    .foregroundStyle(theme.fgMuted)
                    .frame(width: 40, height: 40)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Collapse composer")

            // Send — disabled until there's text, mirroring CaptureBar.
            GrButton(variant: .cta, icon: "corner-down-right", label: "Add", action: submit)
                .disabled(composer.draft.trimmingCharacters(in: .whitespaces).isEmpty)
                .opacity(composer.draft.trimmingCharacters(in: .whitespaces).isEmpty ? 0.5 : 1)
        }
        .padding(.top, 12)
        .padding(.bottom, 16)
        .overlay(alignment: .top) {
            Rectangle().fill(theme.line).frame(height: 1)
        }
        .padding(.top, 6)
    }

    private func errorChip(_ text: String) -> some View {
        Button {
            recorder.dismissError()
        } label: {
            Text(text)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(theme.typeTask)
                .lineLimit(2)
                .frame(maxWidth: .infinity, minHeight: 54, alignment: .topLeading)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func elapsedLabel(_ elapsed: TimeInterval) -> String {
        let total = Int(elapsed)
        return String(format: "%d:%02d", total / 60, total % 60)
    }

    /// Mirrors `CaptureBar.submit`: route through `mosaic.capture(_:target:)`,
    /// clear a child-block target so the next capture doesn't reuse a stale
    /// parent, reset the draft, and collapse.
    private func submit() {
        let trimmed = composer.draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        mosaic.capture(trimmed, target: resolvedTarget)
        if case .childOf = resolvedTarget {
            composer.manualTarget = nil
        }
        composer.draft = ""
        fieldFocused = false
        withAnimation(.snappy(duration: 0.28)) { composer.isExpanded = false }
    }

    /// Mirrors `CaptureBar.toggleRecording`: the finished transcript
    /// arrives via `recorder.lastTranscript`, observed by the shell and
    /// fed into `composer` — not a callback on this view.
    private func toggleRecording() async {
        if isRecording {
            recorder.stop()
            return
        }
        _ = await recorder.start(using: engine)
    }
}
