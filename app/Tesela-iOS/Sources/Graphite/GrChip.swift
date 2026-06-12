import SwiftUI

/// Graphite chip primitive. Mirrors the web `.gr-chip`: rounded rect
/// (radius 8, height 26, h-pad 11) with an optional trailing mono count
/// badge. Inactive = `bg3` fill / `lineSoft` border / `fgMuted` text;
/// active = `accentPrimary @14%` fill / `accentPrimary @40%` border /
/// `accentPrimary` text. The count is mono 10pt (`fgFaint`, accent when
/// active).
struct GrChip: View {
    let label: String
    var active: Bool = false
    var count: Int? = nil
    var action: () -> Void = {}
    /// Optional override for the VoiceOver label. When `nil`, the
    /// visible `label` is announced (preserves the existing primitive
    /// surface for the gallery + view-editor call sites).
    var accessibilityLabelOverride: String? = nil
    /// Optional VoiceOver hint (e.g. "Double-tap to switch to this
    /// view"). When `nil`, no hint is announced — keeps the primitive
    /// quiet for visual-only call sites.
    var accessibilityHint: String? = nil
    /// Stable identifier exposed to UI tests and assistive tech
    /// (e.g. "gr-saved-view-<id>"). When `nil`, SwiftUI derives one.
    var accessibilityIdentifier: String? = nil

    @Environment(\.theme) private var theme

    private var foreground: Color { active ? theme.accentPrimary : theme.fgMuted }
    private var fill: Color { active ? theme.accentPrimary.opacity(0.14) : theme.bg3 }
    private var border: Color { active ? theme.accentPrimary.opacity(0.40) : theme.lineSoft }
    private var countColor: Color { active ? theme.accentPrimary : theme.fgFaint }

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Text(label)
                    .font(.system(size: 12))
                if let count {
                    Text("\(count)")
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(countColor)
                }
            }
            .foregroundStyle(foreground)
            .frame(height: 26)
            .padding(.horizontal, 11)
            .background(fill)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(border, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(accessibilityLabelOverride ?? label)
        .modifier(OptionalAccessibilityHint(hint: accessibilityHint))
        .modifier(OptionalAccessibilityIdentifier(id: accessibilityIdentifier))
    }
}

/// Tiny helper modifiers that no-op when the optional value is `nil`,
/// so primitive call sites stay unannounced and the chip bar can opt in
/// to a label/hint/identifier without wrapping in conditionals.
private struct OptionalAccessibilityHint: ViewModifier {
    let hint: String?
    func body(content: Content) -> some View {
        if let hint {
            content.accessibilityHint(hint)
        } else {
            content
        }
    }
}

private struct OptionalAccessibilityIdentifier: ViewModifier {
    let id: String?
    func body(content: Content) -> some View {
        if let id {
            content.accessibilityIdentifier(id)
        } else {
            content
        }
    }
}
