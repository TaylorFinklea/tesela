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
/// Inbound live-apply of remote splices + cursor remap is DEFERRED to a
/// later increment; this view is the OUTBOUND half. Inbound refreshes
/// stay deferred while editing (`isEditingBlock`), unchanged.
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

        // Drive first-responder state from the binding (replacing the
        // old `@FocusState`). Defer to the next runloop so it doesn't
        // mutate state during a SwiftUI view update.
        if isFocused, !uiView.isFirstResponder {
            DispatchQueue.main.async { uiView.becomeFirstResponder() }
        } else if !isFocused, uiView.isFirstResponder {
            DispatchQueue.main.async { uiView.resignFirstResponder() }
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    final class Coordinator: NSObject, UITextViewDelegate {
        /// Refreshed by `updateUIView` each render so callbacks fire the
        /// latest closures/bindings (the struct is rebuilt every render).
        var parent: CollabTextView
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
}
