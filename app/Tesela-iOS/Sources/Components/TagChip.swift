import SwiftUI

/// Tag chip — trailing-cluster rendering. Mirrors the web's
/// `.cm-tesela-tag-chip` styling: monospace, low-alpha background tint,
/// type-colored foreground. Split parent/leaf so `#nature/birds`
/// renders as a faded `nature/` followed by the bold `birds`.
struct TagChip: View {
    /// Tag value as it appears in source — accepts `#nature/birds`,
    /// `nature/birds`, or just `birds`.
    let value: String
    /// Optional type override; defaults to query green (the same fallback
    /// the web uses for tags that lack a type assignment).
    var typeKind: String = "query"

    @Environment(\.theme) private var theme

    private var parts: (parents: [String], leaf: String) {
        let clean = value.hasPrefix("#") ? String(value.dropFirst()) : value
        var segments = clean.split(separator: "/").map(String.init)
        let leaf = segments.popLast() ?? clean
        return (segments, leaf)
    }

    private var tint: Color {
        theme.typeColor(forKind: typeKind)
    }

    var body: some View {
        HStack(spacing: 0) {
            Text("#").foregroundStyle(theme.fgFaint)
            if !parts.parents.isEmpty {
                Text("\(parts.parents.joined(separator: "/"))/")
                    .foregroundStyle(tint.opacity(0.55))
            }
            Text(parts.leaf)
                .foregroundStyle(tint)
        }
        .font(.system(size: 11.5, weight: .medium, design: .monospaced))
        .padding(.horizontal, 6)
        .padding(.vertical, 1)
        .background(tint.opacity(0.10))
        .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}

/// Inline (non-chip) tag rendering — used when the tag is NOT at a
/// trailing cluster in the block source. Matches the web's `.cm-tesela-tag`
/// styling: primary-colored text with no background pill.
struct InlineTagMark: View {
    let value: String

    @Environment(\.theme) private var theme

    var body: some View {
        Text(value.hasPrefix("#") ? value : "#\(value)")
            .font(.system(size: 14, design: .monospaced))
            .foregroundStyle(theme.accentPrimary.opacity(0.85))
    }
}

/// Wiki-link styling — `[[Page title]]` rendered as a pill-tinted link.
/// Matches `.tx .wlink` from the design tokens.
struct WikiLink: View {
    let title: String

    @Environment(\.theme) private var theme

    var body: some View {
        Text(title)
            .foregroundStyle(theme.accentPrimary)
            .padding(.horizontal, 4)
            .padding(.vertical, 0)
            .background(theme.accentPrimary.opacity(0.12))
            .clipShape(RoundedRectangle(cornerRadius: 2))
    }
}
