import SwiftUI
import UIKit

/// Shared inline-NLP foreground-color painter for the `UITextView`-backed
/// editors (the block editor `CollabTextView` and the capture composer
/// `CaptureTextView`), so both color the to-be-lifted tokens identically.
///
/// Paints `base` everywhere and `highlight` over each matched token span —
/// DISPLAY ONLY: it edits no characters (emits no splice), preserves the
/// selection, and sets `typingAttributes` so text typed AFTER a token isn't
/// painted in the highlight color. Skipped while IME marked text is active so
/// it doesn't fight composition. `ranges` are UTF-16 NSRanges (from
/// `InlineNLP.detectHighlightRanges`); they're re-validated to the text length
/// here so a stale range can never crash `addAttribute`.
enum InlineNLPHighlighter {
    @MainActor
    static func apply(to tv: UITextView, base: UIColor, highlight: UIColor, ranges: [NSRange]) {
        guard tv.markedTextRange == nil else { return }
        let storage = tv.textStorage
        let length = storage.length
        let full = NSRange(location: 0, length: length)
        let valid = ranges.filter {
            $0.location >= 0 && $0.length > 0 && $0.location + $0.length <= length
        }
        // Default typing color so newly typed text after a token isn't painted
        // in the highlight color.
        tv.typingAttributes[.foregroundColor] = base
        let sel = tv.selectedRange
        storage.beginEditing()
        storage.addAttribute(.foregroundColor, value: base, range: full)
        for r in valid {
            storage.addAttribute(.foregroundColor, value: highlight, range: r)
        }
        storage.endEditing()
        tv.selectedRange = sel
    }
}

/// A `UITextView`-backed capture composer field that colors the to-be-lifted
/// inline-NLP tokens (`p2`, `due tomorrow`, …) LIVE as the user types — the
/// same feel as the block editor (`CollabTextView`) — while preserving every
/// behavior the old `TextField(axis: .vertical)` capture field had:
///
///   - the `composer.draft` two-way binding (`text`),
///   - externally-appended voice transcripts (pushed in via the binding while
///     the field is unfocused — see `updateUIView`'s `!isFirstResponder` gate),
///   - the placeholder (a pinned `UILabel`, since `UITextView` has none),
///   - programmatic autofocus (`isFocused` → become/resign first responder,
///     deferred a runloop — the owner sets it after its present transition),
///   - multi-line vertical growth (`sizeThatFits` reports the wrapped height;
///     it scrolls internally past `maxLines`, mirroring `lineLimit(1...12)`).
///
/// It carries NO keyboard accessory and no scroll chrome, so it does NOT change
/// the keyboard frame `CaptureKeyboardObserver` measures — the capture sheet's
/// keyboard avoidance is unaffected. Shared by `GrCaptureSheet` and the legacy
/// `CaptureBar` expanded panel so the two never drift.
struct CaptureTextView: UIViewRepresentable {
    /// The composer draft — two-way bound (the field is the source of truth
    /// while focused; the binding is the source for external writes like voice).
    @Binding var text: String
    /// Drives first-responder state (autofocus). The owner sets it true after
    /// its present transition; the coordinator flips it false on blur.
    @Binding var isFocused: Bool

    var placeholder: String
    var textColor: Color
    var tintColor: Color
    var placeholderColor: Color
    var fontSize: CGFloat = 16

    /// Inline-NLP highlight spans for the current text, GATED on the picked
    /// capture type by the owner (no type picked → returns `[]` → no coloring).
    /// Read live each call so changing the type picker recolors on the next
    /// `updateUIView`. Must match the add-time lift's gating exactly.
    var nlpHighlightRanges: (String) -> [NSRange]
    /// Color drawn over a matched token span (defaults to `tintColor`).
    var nlpHighlightColor: Color? = nil

    /// Visible lines before the field scrolls internally (mirrors the old
    /// `lineLimit(1...12)` cap so a long capture doesn't grow without bound).
    static let maxLines = 12

    func makeUIView(context: Context) -> UITextView {
        let tv = UITextView()
        tv.delegate = context.coordinator
        tv.font = .systemFont(ofSize: fontSize)
        tv.backgroundColor = .clear
        tv.textColor = UIColor(textColor)
        tv.tintColor = UIColor(tintColor)
        // Grow with content inside the surrounding layout (mirrors
        // `TextField(axis: .vertical)`); scrolling re-enables past `maxLines`.
        tv.isScrollEnabled = false
        tv.textContainerInset = .zero
        tv.textContainer.lineFragmentPadding = 0
        tv.autocorrectionType = .default
        tv.autocapitalizationType = .sentences
        tv.text = text

        // Placeholder: a label pinned to the text origin. `textContainerInset`
        // is zero and `lineFragmentPadding` 0, so top/leading align with the
        // first glyph.
        let ph = UILabel()
        ph.text = placeholder
        ph.font = tv.font
        ph.textColor = UIColor(placeholderColor)
        ph.numberOfLines = 0
        ph.translatesAutoresizingMaskIntoConstraints = false
        tv.addSubview(ph)
        NSLayoutConstraint.activate([
            ph.topAnchor.constraint(equalTo: tv.topAnchor),
            ph.leadingAnchor.constraint(equalTo: tv.leadingAnchor),
            ph.trailingAnchor.constraint(lessThanOrEqualTo: tv.trailingAnchor),
        ])
        ph.isHidden = !text.isEmpty
        context.coordinator.placeholderLabel = ph

        context.coordinator.applyHighlight(tv)
        return tv
    }

    func updateUIView(_ uiView: UITextView, context: Context) {
        // Refresh the captured `parent` so the coordinator's callbacks fire the
        // LATEST closures/bindings (SwiftUI rebuilds this struct every render).
        context.coordinator.parent = self
        // Don't stomp in-flight typing/caret: only push the binding into the
        // field when it genuinely diverges AND we aren't the first responder —
        // e.g. a voice transcript appended into `draft` while unfocused.
        if !uiView.isFirstResponder, uiView.text != text {
            uiView.text = text
        }
        uiView.textColor = UIColor(textColor)
        uiView.tintColor = UIColor(tintColor)
        context.coordinator.placeholderLabel?.text = placeholder
        context.coordinator.placeholderLabel?.textColor = UIColor(placeholderColor)
        context.coordinator.placeholderLabel?.isHidden = !(uiView.text ?? "").isEmpty

        // Drive first-responder state from the binding (deferred a runloop so it
        // doesn't mutate state during a SwiftUI update).
        if isFocused, !uiView.isFirstResponder {
            DispatchQueue.main.async { uiView.becomeFirstResponder() }
        } else if !isFocused, uiView.isFirstResponder {
            DispatchQueue.main.async { uiView.resignFirstResponder() }
        }

        // Recolor — picks up a type-picker change (the gating closure changed)
        // even when the text itself didn't.
        context.coordinator.applyHighlight(uiView)
    }

    /// Report the wrapped height for the proposed width so the field grows
    /// vertically (the sheet expands) instead of clipping a single line. Caps at
    /// `maxLines` and enables internal scrolling past it.
    func sizeThatFits(_ proposal: ProposedViewSize, uiView: UITextView, context: Context) -> CGSize? {
        guard let width = proposal.width, width > 0, width.isFinite else { return nil }
        let fitted = uiView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude))
        let lineH = (uiView.font ?? .systemFont(ofSize: fontSize)).lineHeight
        let maxH = ceil(lineH * CGFloat(Self.maxLines))
        let h = ceil(fitted.height)
        let shouldScroll = h > maxH
        if uiView.isScrollEnabled != shouldScroll {
            DispatchQueue.main.async { uiView.isScrollEnabled = shouldScroll }
        }
        return CGSize(width: width, height: min(h, maxH))
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, UITextViewDelegate {
        /// Refreshed by `updateUIView` each render so callbacks use the latest
        /// closures/bindings (the struct is rebuilt every render).
        var parent: CaptureTextView
        weak var placeholderLabel: UILabel?

        init(_ parent: CaptureTextView) { self.parent = parent }

        func applyHighlight(_ tv: UITextView) {
            let base = UIColor(parent.textColor)
            let highlight = UIColor(parent.nlpHighlightColor ?? parent.tintColor)
            InlineNLPHighlighter.apply(
                to: tv, base: base, highlight: highlight,
                ranges: parent.nlpHighlightRanges(tv.text ?? ""))
        }

        func textViewDidChange(_ textView: UITextView) {
            parent.text = textView.text ?? ""
            placeholderLabel?.isHidden = !(textView.text ?? "").isEmpty
            applyHighlight(textView)
        }

        func textViewDidEndEditing(_ textView: UITextView) {
            if parent.isFocused { parent.isFocused = false }
        }
    }
}
