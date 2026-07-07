import SwiftUI
import UIKit

/// A `UITextView`-backed block editor that emits **character-level
/// splices** on each edit instead of re-authoring the whole block text.
///
/// Why this exists (collab editing C1 outbound, 2026-06-03): the block
/// text lives in a per-block Loro `LoroText` (`text_seq`). When two
/// devices edit the SAME block, a whole-text re-author Myers-diffs the
/// local string against the peer's value and emits DELETE ops for the
/// peer's characters → the peer's edit is clobbered. The CRDT merges
/// character *splices* (insert/delete at an offset), so the client must
/// ship the user's actual keystroke, not the whole string. `UITextView`
/// exposes the edit as an `NSRange` + replacement in
/// `shouldChangeTextIn`, which is exactly the (utf16Offset, deleteLen,
/// insert) triple the engine's `spliceBlockText` wants — `NSRange` is
/// UTF-16, matching `text_seq`'s UTF-16 splice offsets.
///
/// Offset-alignment contract: `text` MUST be loaded as a faithful 1:1
/// view of the engine's stored block text (the materialized line's
/// visible content — tags inline, bullet + bid comment excluded). While
/// editing, splices are the ONLY in-block text path — no whole-text
/// normalizing writeback runs in between — so the editor's offsets stay
/// aligned with the engine's `text_seq`. See `BlockRow`/`MockMosaicService`.
///
/// Inbound live-apply (collab editing C1-inbound, 2026-06-03): when a
/// remote peer splices the SAME block while it's open, the engine merges
/// the edit and `MockMosaicService` reads the merged text and calls
/// `CollabTextInserter.reconcile(toEngineText:)`, which applies a minimal
/// diff to the live `UITextView` and remaps the caret — so the peer's
/// characters appear under the cursor without clobbering in-flight typing.
/// The non-editing blocks still refresh via the deferred full-note path.
/// One remote peer's caret to render in a block (Phase 3 presence).
struct RemoteCaret: Equatable {
    let offset: Int
    let color: Color
    /// The peer's friendly device name (when it sent one), drawn as a tiny
    /// caret flag and used by the block-level presence chip.
    var name: String? = nil
    /// The sending peer id — a stable fallback label (short prefix) when the
    /// peer sent no name, and a unique key for the chip cluster.
    var peer: String? = nil
}

/// Theme colors for JQL syntax kinds inside `query::` lines (tesela-vp9.6),
/// one per `QueryAuthoring.PreviewTokenKind` case. Mirrors
/// `GrViewEditorSheet.previewColor(for:)`'s theme mapping (key→
/// accentSecondary, operator→fgMuted, value→fgDefault, string→typeNote,
/// number→typeProject, paren→fgFaint) so the saved-view editor's
/// token-preview row and this in-block highlight read as the same palette.
struct JQLThemeColors {
    let key: Color
    let operatorKind: Color
    let value: Color
    let string: Color
    let number: Color
    let paren: Color

    func color(for kind: QueryAuthoring.PreviewTokenKind) -> Color {
        switch kind {
        case .key: return key
        case .operatorKind: return operatorKind
        case .value: return value
        case .string: return string
        case .number: return number
        case .paren: return paren
        }
    }
}

/// Paints `JQLLineHighlight.HighlightSpan`s (tesela-vp9.6) — the syntax
/// colors for `query::` lines — OVER whatever `InlineNLPHighlighter.apply`
/// already painted in the SAME pass. Deliberately does NOT reset the base
/// color the way `InlineNLPHighlighter.apply` does: `query::` lines carry
/// no NLP spans by construction (`CollabTextView.Coordinator
/// .applyNLPHighlight` filters them out before that first pass runs), so
/// they're already uniformly `base`-colored by the time this runs — this
/// only needs to ADD the JQL foreground colors on top.
///
/// Kept as a sibling to `InlineNLPHighlighter` rather than a case added to
/// its `InlineNLP.HighlightKind` vocabulary: JQL syntax roles (key/
/// operator/value/string/number/paren) are a different axis than the
/// NLP-lift semantic kinds (priority level / date), and extending
/// `HighlightKind` would force every `colorForKind` switch — including the
/// unrelated capture composer's (`CaptureTextView.swift`), which never
/// produces a `query::` line — to exhaustively handle cases it never
/// emits. This keeps `InlineNLPHighlighter.apply`'s mechanics (and every
/// existing call site) completely untouched. Same selection-preserving /
/// IME-composition-skipping / stale-range-safe posture as
/// `InlineNLPHighlighter.apply`.
enum JQLHighlighter {
    @MainActor
    static func overlay(
        on tv: UITextView,
        spans: [JQLLineHighlight.HighlightSpan],
        colorForKind: (QueryAuthoring.PreviewTokenKind) -> UIColor
    ) {
        guard !spans.isEmpty, tv.markedTextRange == nil else { return }
        let storage = tv.textStorage
        let length = storage.length
        let valid = spans.filter {
            $0.range.location >= 0 && $0.range.length > 0 && $0.range.location + $0.range.length <= length
        }
        guard !valid.isEmpty else { return }
        let sel = tv.selectedRange
        storage.beginEditing()
        for span in valid {
            storage.addAttribute(.foregroundColor, value: colorForKind(span.kind), range: span.range)
        }
        storage.endEditing()
        tv.selectedRange = sel
    }
}

struct CollabTextView: UIViewRepresentable {
    /// The block's raw text — the engine-exact stored value. Bound so
    /// SwiftUI and the `UITextView` agree on the current string; the
    /// view updates it in `textViewDidChange` after the splice applies.
    @Binding var text: String
    /// Drives first-responder state. Set true to focus the field; the
    /// coordinator flips it false on blur so the parent can react
    /// (commit) exactly as the old `@FocusState` editField did.
    @Binding var isFocused: Bool

    /// Theme tokens, matching the previous `editField` (size-15 body,
    /// `fgDefault` text, `accentPrimary` caret).
    var textColor: Color
    var tintColor: Color

    /// One local keystroke as a UTF-16 splice: delete `utf16DeleteLen`
    /// code units at `utf16Offset`, then insert `insert`. Routed to the
    /// engine's `spliceBlockText`. Called for EVERY in-field text change,
    /// including toolbar insertions (which go through `insertAtCaret`).
    var onSplice: (_ utf16Offset: Int, _ utf16DeleteLen: Int, _ insert: String) -> Void
    /// Fires once when editing finishes (blur). Mirrors the old
    /// `onCommitEdit` blur-commit. Carries the field's final text.
    var onCommit: (String) -> Void
    /// "Enter on an empty line → split": when a `\n` produces a trailing
    /// double newline, the parent commits this block (stripped) and
    /// appends a new sibling. Replicates the old `editBuffer` `\n\n`
    /// heuristic so the keyboard Return key still splits blocks.
    var onSplitToNewBlock: (String) -> Void

    /// Imperative seam so the keyboard toolbar's text-inserting buttons
    /// (`[[]]`, `#`, `/`) insert at the caret THROUGH the splice path
    /// (rather than mutating a separate buffer), keeping the editor and
    /// engine `text_seq` aligned. `BlockRow` holds this and calls it from
    /// the toolbar handlers.
    let inserter: CollabTextInserter

    /// Drives the inline suggestion strip ([[ links / # tags / slash) in the
    /// keyboard accessory. The coordinator updates it as the user types;
    /// `BlockRow` renders `results` and commits a pick through `inserter`.
    var autocomplete: EditorAutocomplete

    /// The formatting accessory hosted as the text view's
    /// `inputAccessoryView`. SwiftUI's `ToolbarItemGroup(placement:
    /// .keyboard)` only attaches when the first responder is a
    /// SwiftUI-managed text input — this editor's responder is a raw
    /// `UITextView`, so the `.toolbar` route silently shows nothing
    /// (the bug: today's collab-edited blocks had no toolbar while
    /// yesterday's legacy `TextField` did). Hosting the same accessory
    /// content here is the only attachment point UIKit honors.
    var accessory: AnyView? = nil

    /// Inline-NLP highlight (iOS surface parity): returns the UTF-16 spans of
    /// the to-be-lifted tokens (`p2`, `due tomorrow`, …) in the current text so
    /// the editor can color them live as the user types — the same spans
    /// `InlineNLP.detectLifts` strips on commit. `nil` (or empty) → no
    /// highlight (plain prose / a block whose type declares no NLP). Read live
    /// each call so a type-page edit mid-session takes effect on the next change.
    var nlpHighlightRanges: ((String) -> [InlineNLP.HighlightSpan])? = nil
    /// Foreground color drawn over a matched non-priority/non-date NLP token
    /// span (e.g. a `/status` verb) — the pre-existing single-accent
    /// behavior. Defaults to the tint accent. The non-token text uses
    /// `textColor`.
    var nlpHighlightColor: Color? = nil
    /// Semantic priority colors keyed 1...4 (Taylor's locked direction,
    /// tesela-b1s: p1 red, p2 yellow, p3 blue, p4 gray). Falls back to
    /// `nlpHighlightColor` for a level with no entry.
    var nlpPriorityColors: [Int: Color] = [:]
    /// Color drawn over a matched date phrase (cyan, matching the desktop
    /// block editor). Falls back to `nlpHighlightColor` when `nil`.
    var nlpDateColor: Color? = nil
    /// JQL syntax highlight colors for `query::` lines (tesela-vp9.6) —
    /// key/operator/value/string/number/paren, keyed by
    /// `QueryAuthoring.PreviewTokenKind`. `nil` (default; e.g. the capture
    /// composer never sets this) → no JQL coloring, `query::` lines then
    /// fall through to ordinary NLP-lift detection like any other line.
    /// When set, `query::` lines get THESE colors INSTEAD of the NLP-lift
    /// colors above — see `Coordinator.applyNLPHighlight`.
    var jqlColors: JQLThemeColors? = nil

    /// Phase 3 presence: fires with the caret's utf16 offset whenever it moves
    /// (tap / arrow / typing). The owner publishes it as a presence frame.
    var onCaretMove: ((Int) -> Void)? = nil
    /// Phase 3 presence: OTHER peers' carets to draw in this block. Re-applied
    /// every render (the store drives re-renders), so an idle peer's caret
    /// recomputes against the current text.
    var remoteCarets: [RemoteCaret] = []

    /// Height of the hosted accessory bar (pill + its vertical padding).
    /// `BlockRow.collabKeyboardAccessory`'s layout must add up to this.
    static let accessoryBarHeight: CGFloat = 54

    func makeUIView(context: Context) -> UITextView {
        let tv = UITextView()
        tv.delegate = context.coordinator
        tv.font = .systemFont(ofSize: 15)
        tv.backgroundColor = .clear
        tv.textColor = UIColor(textColor)
        tv.tintColor = UIColor(tintColor)
        // Match the SwiftUI body layout: no inset chrome, scrolling
        // disabled so the text view grows with its content inside the
        // surrounding ScrollView (mirrors `TextField(axis: .vertical)`).
        tv.isScrollEnabled = false
        tv.textContainerInset = .zero
        tv.textContainer.lineFragmentPadding = 0
        tv.autocorrectionType = .default
        tv.autocapitalizationType = .sentences
        tv.text = text
        // Install the keyboard accessory as a UIKit `inputAccessoryView`
        // (see `accessory` doc — SwiftUI's `.toolbar(.keyboard)` never
        // attaches to a UIKit first responder). The hosting controller
        // lives on the coordinator so `updateUIView` can refresh its
        // rootView when the theme / configured items change.
        if let accessory {
            let host = UIHostingController(rootView: accessory)
            host.view.backgroundColor = .clear
            // Don't let the hosted view apply keyboard/safe-area
            // avoidance — it IS the keyboard accessory.
            host.safeAreaRegions = []
            host.view.frame = CGRect(
                x: 0, y: 0, width: 0, height: Self.accessoryBarHeight
            )
            tv.inputAccessoryView = host.view
            context.coordinator.accessoryHost = host
        }
        // Bind the imperative inserter to this concrete text view so
        // toolbar buttons insert at the live caret.
        inserter.bind(tv, coordinator: context.coordinator)
        return tv
    }

    func updateUIView(_ uiView: UITextView, context: Context) {
        // Keep the live text view as the source of truth for the string
        // while focused — we must NOT stomp the user's in-flight text or
        // caret with a stale binding value. Only reconcile when the
        // binding genuinely diverges AND the field isn't the active
        // first responder (e.g. the row re-rendered with a fresh value
        // before focus). This avoids fighting the keyboard/IME.
        if !uiView.isFirstResponder, uiView.text != text {
            uiView.text = text
        }
        uiView.textColor = UIColor(textColor)
        uiView.tintColor = UIColor(tintColor)
        // Refresh the coordinator's captured `parent` so its delegate
        // callbacks invoke the LATEST closures/bindings — SwiftUI rebuilds
        // this struct on every render, but `makeCoordinator()` ran once.
        context.coordinator.parent = self
        inserter.bind(uiView, coordinator: context.coordinator)
        // Refresh the hosted accessory so theme changes / toolbar
        // reconfiguration (Settings) propagate into the UIKit bar.
        if let accessory {
            context.coordinator.accessoryHost?.rootView = accessory
        }

        // Drive first-responder state from the binding (replacing the
        // old `@FocusState`). Defer to the next runloop so it doesn't
        // mutate state during a SwiftUI view update.
        if isFocused, !uiView.isFirstResponder {
            DispatchQueue.main.async { uiView.becomeFirstResponder() }
        } else if !isFocused, uiView.isFirstResponder {
            DispatchQueue.main.async { uiView.resignFirstResponder() }
        }

        renderRemoteCarets(on: uiView)
        context.coordinator.applyNLPHighlight(uiView)
    }

    /// Draw OTHER peers' carets as thin colored bars at their offsets, each
    /// topped by a tiny name flag (mirrors web `cm-remote-cursor-flag`). Cheap +
    /// idempotent: clears prior remote-caret layers and re-adds from the current
    /// `remoteCarets`. `caretRect` is in the text view's own layer space, so no
    /// coordinate conversion is needed.
    private func renderRemoteCarets(on tv: UITextView) {
        tv.layer.sublayers?
            .filter { $0.name == "remoteCaret" }
            .forEach { $0.removeFromSuperlayer() }
        guard !remoteCarets.isEmpty else { return }
        let len = (tv.text as NSString?)?.length ?? 0
        for caret in remoteCarets {
            let off = max(0, min(caret.offset, len))
            guard let pos = tv.position(from: tv.beginningOfDocument, offset: off) else { continue }
            let rect = tv.caretRect(for: pos)
            guard rect.height > 0, rect.minX.isFinite, rect.minY.isFinite else { continue }
            let color = UIColor(caret.color)
            let bar = CALayer()
            bar.name = "remoteCaret"
            bar.backgroundColor = color.cgColor
            bar.frame = CGRect(x: rect.minX, y: rect.minY, width: 2, height: rect.height)
            bar.opacity = 0.85
            tv.layer.addSublayer(bar)
            // Tiny name flag above the caret (truncated; secondary to the
            // block-level chip but matches the web's per-caret label).
            if let raw = caret.name?.trimmingCharacters(in: .whitespacesAndNewlines), !raw.isEmpty {
                let label = raw.count > 12 ? String(raw.prefix(11)) + "\u{2026}" : raw
                let font = UIFont.systemFont(ofSize: 9, weight: .semibold)
                let textW = ceil((label as NSString).size(withAttributes: [.font: font]).width)
                let flagH: CGFloat = 12
                let flag = CATextLayer()
                flag.name = "remoteCaret"
                flag.string = label
                flag.font = font
                flag.fontSize = 9
                flag.alignmentMode = .center
                flag.foregroundColor = UIColor.white.cgColor
                flag.backgroundColor = color.cgColor
                flag.cornerRadius = 2
                flag.masksToBounds = true
                flag.contentsScale = tv.window?.screen.scale ?? UIScreen.main.scale
                flag.frame = CGRect(
                    x: rect.minX, y: max(0, rect.minY - flagH),
                    width: textW + 6, height: flagH)
                tv.layer.addSublayer(flag)
            }
        }
    }

    /// Size the editor to the proposed WIDTH and report the wrapped height,
    /// so this non-scrolling UITextView wraps to the row width and grows
    /// vertically (the row expands) instead of laying out a single clipped
    /// line. Without this, SwiftUI measures the text view at its unbounded
    /// single-line intrinsic width and the text runs off / clips. (iOS 16+.)
    func sizeThatFits(_ proposal: ProposedViewSize, uiView: UITextView, context: Context) -> CGSize? {
        guard let width = proposal.width, width > 0, width.isFinite else { return nil }
        let fitted = uiView.sizeThatFits(
            CGSize(width: width, height: .greatestFiniteMagnitude)
        )
        return CGSize(width: width, height: ceil(fitted.height))
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    final class Coordinator: NSObject, UITextViewDelegate {
        /// Refreshed by `updateUIView` each render so callbacks fire the
        /// latest closures/bindings (the struct is rebuilt every render).
        var parent: CollabTextView
        /// Retains the keyboard-accessory hosting controller for the
        /// lifetime of the text view (its view is the
        /// `inputAccessoryView`); `updateUIView` swaps its rootView.
        var accessoryHost: UIHostingController<AnyView>?
        /// Guards against re-entrant split handling while we tear down
        /// the field for a block split.
        private var didSplit = false

        init(_ parent: CollabTextView) {
            self.parent = parent
        }

        /// The user (or autocorrect/IME) is about to replace `range`
        /// with `replacementText`. `range` is UTF-16 (NSRange), exactly
        /// what `spliceBlockText` consumes. Emit the splice, then return
        /// `true` so UITextView applies it locally; `textViewDidChange`
        /// then syncs the binding.
        func textView(
            _ textView: UITextView,
            shouldChangeTextIn range: NSRange,
            replacementText text: String
        ) -> Bool {
            // "Enter on an empty line → split": replicate the old
            // `editBuffer.hasSuffix("\n\n")` heuristic. A `\n` insertion
            // at the end of a string that already ends in `\n` produces a
            // trailing double newline — treat it as the split gesture
            // instead of inserting the newline.
            if text == "\n" {
                let current = textView.text ?? ""
                let ns = current as NSString
                let atEnd = range.location + range.length >= ns.length
                let endsWithNewline = current.hasSuffix("\n")
                if atEnd && endsWithNewline {
                    guard !didSplit else { return false }
                    didSplit = true
                    // The FIRST Return already spliced one trailing `\n`
                    // into the engine's `text_seq`; this second Return is
                    // the split gesture. Splice OUT only the TRAILING
                    // whitespace/newline run so the engine block ends
                    // clean — the splice path stays the single source of
                    // truth, no whole-text re-author. Trim only the
                    // trailing run (not leading) so the UTF-16 delete
                    // offset/length stay exact.
                    let trimmedSet = CharacterSet.whitespacesAndNewlines
                    var bodyLen = ns.length
                    while bodyLen > 0,
                          let scalar = Unicode.Scalar(ns.character(at: bodyLen - 1)),
                          trimmedSet.contains(scalar) {
                        bodyLen -= 1
                    }
                    let body = ns.substring(to: bodyLen)
                    let trailingLen = ns.length - bodyLen
                    if trailingLen > 0 {
                        parent.onSplice(bodyLen, trailingLen, "")
                    }
                    parent.onSplitToNewBlock(body)
                    return false
                }
            }
            parent.onSplice(range.location, range.length, text)
            return true
        }

        func textViewDidChange(_ textView: UITextView) {
            parent.text = textView.text ?? ""
            updateAutocomplete(textView)
            applyNLPHighlight(textView)
        }

        /// Color the to-be-lifted inline-NLP tokens (`p2`, `due tomorrow`, …)
        /// live, mirroring the web editor's inline highlight so the user sees
        /// what will be parsed. Recomputes the spans from the same gated
        /// detector the lift uses, then paints ONLY foreground-color attributes
        /// on the text storage — no character edit, so it emits no splice and
        /// keeps the engine `text_seq` aligned. Caret/selection are preserved.
        /// Skipped during IME composition (marked text) to avoid fighting it.
        ///
        /// tesela-vp9.6: `query::` lines get JQL syntax coloring
        /// (`JQLLineHighlight.detectSpans` — key/operator/value/string/
        /// number/paren) INSTEAD of NLP-lift coloring. NLP spans that fall
        /// on a `query::` line are filtered out here BEFORE the shared
        /// painter runs, so a date word inside a quoted query value
        /// (`query:: title LIKE "tomorrow"`) never reads as an NLP date
        /// token. The two passes coexist without fighting: the first
        /// (`InlineNLPHighlighter.apply`, unmodified) resets the WHOLE
        /// text to `base` and paints the (filtered) NLP spans; the second
        /// (`JQLHighlighter.overlay`) only ADDS the JQL spans' colors on
        /// top — `query::` lines carry no NLP spans by construction, so
        /// nothing the first pass painted there needs undoing.
        func applyNLPHighlight(_ tv: UITextView) {
            guard parent.nlpHighlightRanges != nil || parent.jqlColors != nil else { return }
            let text = tv.text ?? ""
            let jqlLineRanges = parent.jqlColors != nil ? JQLLineHighlight.queryLineRanges(in: text) : []
            let rawNlpSpans = parent.nlpHighlightRanges?(text) ?? []
            let nlpSpans = jqlLineRanges.isEmpty ? rawNlpSpans : rawNlpSpans.filter { span in
                !jqlLineRanges.contains { NSIntersectionRange($0, span.range).length > 0 }
            }

            let base = UIColor(parent.textColor)
            let fallback = UIColor(parent.nlpHighlightColor ?? parent.tintColor)
            let priorityColors = parent.nlpPriorityColors
            let dateColor = parent.nlpDateColor.map(UIColor.init)
            // Shared painter (also used by the capture composer) so both
            // surfaces color identically. Preserves selection/typingAttributes
            // and skips IME composition — same as the prior inline body.
            InlineNLPHighlighter.apply(
                to: tv, base: base, spans: nlpSpans
            ) { kind in
                switch kind {
                case .priority(let level):
                    return priorityColors[level].map(UIColor.init) ?? fallback
                case .date:
                    return dateColor ?? fallback
                case .other:
                    return fallback
                }
            }

            if let jqlColors = parent.jqlColors {
                let jqlSpans = JQLLineHighlight.detectSpans(in: text)
                JQLHighlighter.overlay(on: tv, spans: jqlSpans) { UIColor(jqlColors.color(for: $0)) }
            }
        }

        func textViewDidChangeSelection(_ textView: UITextView) {
            // Phase 3 presence: any caret move (tap / arrow / post-typing)
            // publishes our caret to peers.
            let caret = textView.selectedRange.location + textView.selectedRange.length
            parent.onCaretMove?(caret)
            // Caret moved without a text change (tap / arrow) — re-check
            // whether the caret still sits inside an open `[[…`.
            updateAutocomplete(textView)
        }

        /// Open / refresh / close the inline suggestions ([[ / # / slash).
        private func updateAutocomplete(_ tv: UITextView) {
            guard tv.markedTextRange == nil else { return }  // skip IME composition
            let sel = tv.selectedRange
            let caret = sel.location + sel.length
            let text = tv.text ?? ""
            if let hit = LinkSuggest.detectTrigger(in: text, caretUTF16: caret) {
                parent.autocomplete.update(kind: hit.kind, start: hit.start, query: hit.query)
            } else if let nlp = parent.autocomplete.nlpDetector?(text, caret) {
                // No explicit trigger open — offer an inline-NLP lift if the
                // just-typed token/tail is a confident property/date match.
                parent.autocomplete.updateNLP(nlp)
            } else {
                parent.autocomplete.dismiss()
            }
        }

        func textViewDidEndEditing(_ textView: UITextView) {
            parent.isFocused = false
            parent.onCommit(textView.text ?? "")
        }
    }
}

/// Imperative handle that lets `BlockRow`'s keyboard toolbar insert text
/// at the live caret while routing the change through the same
/// `shouldChangeTextIn` → `onSplice` path the keyboard uses, so the
/// editor and engine `text_seq` never desync. Held by `BlockRow` as a
/// `@StateObject`-free reference type and bound to the concrete
/// `UITextView` in `makeUIView`.
final class CollabTextInserter {
    private weak var textView: UITextView?
    private weak var coordinator: CollabTextView.Coordinator?

    func bind(_ textView: UITextView, coordinator: CollabTextView.Coordinator) {
        self.textView = textView
        self.coordinator = coordinator
    }

    /// Insert `string` at the current caret (or replace the selection),
    /// going through the delegate's `shouldChangeTextIn` so the splice is
    /// emitted exactly as a keystroke would be. No-op if the field isn't
    /// bound/focused yet.
    func insertAtCaret(_ string: String) {
        guard let tv = textView, let selected = tv.selectedTextRange else { return }
        let range = tv.selectedRange  // UTF-16 NSRange of the caret/selection
        // Ask the delegate (mirrors what UIKit does for a keystroke) so
        // the splice fires; if it returns true, apply the edit + advance
        // the caret past the inserted text.
        let shouldChange = coordinator?.textView(
            tv,
            shouldChangeTextIn: range,
            replacementText: string
        ) ?? true
        guard shouldChange else { return }
        tv.replace(selected, withText: string)
        // `textViewDidChange` fires from `replace(_:withText:)` and syncs
        // the binding. Caret advances to the end of the inserted text
        // automatically.
        _ = tv
    }

    /// Replace the `[[query` span — from `startOffset` (UTF-16) to the live
    /// caret — with `string` (the chosen `[[Page]]`), routed through the
    /// delegate's `shouldChangeTextIn` so the splice is emitted and the
    /// engine's `text_seq` stays aligned. Used by the link-autocomplete pick.
    func replaceTrigger(startOffset: Int, with string: String) {
        guard let tv = textView else { return }
        let caret = tv.selectedRange.location + tv.selectedRange.length
        let len = max(0, caret - startOffset)
        guard startOffset >= 0, startOffset + len <= (tv.text as NSString?)?.length ?? 0 else { return }
        let range = NSRange(location: startOffset, length: len)
        let shouldChange = coordinator?.textView(
            tv, shouldChangeTextIn: range, replacementText: string
        ) ?? true
        guard shouldChange else { return }
        if let start = tv.position(from: tv.beginningOfDocument, offset: range.location),
           let end = tv.position(from: start, offset: range.length),
           let textRange = tv.textRange(from: start, to: end) {
            tv.replace(textRange, withText: string)
        }
    }

    /// Inbound live-apply (collab editing C1-inbound): make the live text
    /// view match the engine's post-apply block text `newText` — the MERGED
    /// result after a remote peer's concurrent splice landed — with a
    /// MINIMAL replacement (common UTF-16 prefix/suffix preserved) and a
    /// caret remap, so the user's cursor stays put relative to their own
    /// text while the peer's characters appear.
    ///
    /// No-op when the field already equals `newText`: this covers the echo
    /// of our OWN splice (engine + view already agree) and any redundant
    /// delta, so it's safe to call on every inbound apply. The text is set
    /// programmatically (NOT through `shouldChangeTextIn`), so it does NOT
    /// emit an outbound splice; the SwiftUI binding is synced so the block's
    /// edit buffer + blur-commit reflect the merged text. Skipped while IME
    /// marked text is active (composition) — the post-blur refresh
    /// reconciles those.
    func reconcile(toEngineText newText: String) {
        guard let tv = textView else { return }
        if tv.markedTextRange != nil { return }
        let old = tv.text ?? ""
        if old == newText { return }
        let oldU = Array(old.utf16)
        let newU = Array(newText.utf16)
        // Common prefix.
        var pre = 0
        let cap = min(oldU.count, newU.count)
        while pre < cap, oldU[pre] == newU[pre] { pre += 1 }
        // Common suffix (not overlapping the shared prefix on either side).
        var suf = 0
        while suf < (oldU.count - pre), suf < (newU.count - pre),
              oldU[oldU.count - 1 - suf] == newU[newU.count - 1 - suf] { suf += 1 }
        let removed = oldU.count - pre - suf  // UTF-16 units the remote edit removed
        let inserted = newU.count - pre - suf  // UTF-16 units it added
        let delta = inserted - removed
        let changeEnd = pre + removed
        // Remap a caret/anchor position through the changed span. A PURE
        // insertion exactly AT the caret uses right-gravity: the caret
        // advances PAST the peer's inserted text so the user keeps typing
        // after it (and concurrent same-spot edits stack in arrival order)
        // rather than being stranded before it.
        func remap(_ pos: Int) -> Int {
            if pos < pre { return pos }                          // strictly before — unchanged
            if pos == pre, removed == 0 { return pos + inserted } // insertion at caret — advance past it
            if pos >= changeEnd { return pos + delta }           // after the change — shift by net delta
            return pre + inserted                                 // inside a replaced span — clamp to its end
        }
        let sel = tv.selectedRange
        let loc = remap(sel.location)
        let end = remap(sel.location + sel.length)
        // Apply programmatically — skips `shouldChangeTextIn`, so no echo splice.
        tv.text = newText
        let clampedLoc = max(0, min(loc, newU.count))
        let clampedLen = max(0, min(end - loc, newU.count - clampedLoc))
        tv.selectedRange = NSRange(location: clampedLoc, length: clampedLen)
        // Sync the SwiftUI binding so editBuffer / commit-on-blur reflect the
        // merged text. `updateUIView` won't re-stomp the field: it's gated on
        // `!isFirstResponder` and the field is focused.
        coordinator?.parent.text = newText
    }
}
