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

    /// Drives the `[[` page-link suggestions strip in the keyboard
    /// accessory. The coordinator updates it as the user types; `BlockRow`
    /// renders `results` and commits a pick through `inserter`.
    var autocomplete: LinkAutocomplete

    /// The formatting accessory hosted as the text view's
    /// `inputAccessoryView`. SwiftUI's `ToolbarItemGroup(placement:
    /// .keyboard)` only attaches when the first responder is a
    /// SwiftUI-managed text input — this editor's responder is a raw
    /// `UITextView`, so the `.toolbar` route silently shows nothing
    /// (the bug: today's collab-edited blocks had no toolbar while
    /// yesterday's legacy `TextField` did). Hosting the same accessory
    /// content here is the only attachment point UIKit honors.
    var accessory: AnyView? = nil

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
        // Wrap to the available width instead of laying out one ever-widening
        // line. A non-scrolling UITextView in SwiftUI reports its full
        // single-line text as its intrinsic width and resists horizontal
        // compression, so without lowering that resistance the row grows off
        // the right edge instead of wrapping. (text_seq offsets are unchanged
        // — this is layout only.)
        tv.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
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
        }

        func textViewDidChangeSelection(_ textView: UITextView) {
            // Caret moved without a text change (tap / arrow) — re-check
            // whether the caret still sits inside an open `[[…`.
            updateAutocomplete(textView)
        }

        /// Open / refresh / close the `[[` link suggestions for the caret.
        private func updateAutocomplete(_ tv: UITextView) {
            guard tv.markedTextRange == nil else { return }  // skip IME composition
            let sel = tv.selectedRange
            let caret = sel.location + sel.length
            if let hit = LinkSuggest.detectQuery(in: tv.text ?? "", caretUTF16: caret) {
                parent.autocomplete.update(start: hit.start, query: hit.query)
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
