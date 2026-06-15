import SwiftUI

/// Deterministic per-tag color — the iOS port of web `tag-color.ts` (the
/// "colored per-tag pills" redesign, decided 2026-06-07). A tag's hue is a
/// stable FNV-1a hash of its lowercased name into a curated palette, so tags
/// are scannable across a list with zero config. A tag page's `color::`
/// frontmatter (a `#rrggbb` hex or a named hue) overrides the hash when
/// threaded through `colorOverride`.
enum TagPalette {
    /// Curated, theme-harmonious hues — kept in lockstep with the web PALETTE
    /// in `tag-color.ts` so a tag is the same color on every surface.
    static let hues: [UInt32] = [
        0xE8697F, // rose / task
        0x62B8CE, // teal / event
        0xE4AE66, // amber / note
        0x7493E8, // blue / project
        0xAE90E6, // violet / person
        0x85BC63, // green / query
        0xFF6B5A, // coral (brand)
        0xE093C4, // pink
        0x6FC3A8, // mint
        0xC9A24B, // gold
    ]

    /// Named-hue aliases for a tag page's `color::` override — mirrors the web
    /// `NAMED` map so `color:: coral` resolves identically on both platforms.
    static let named: [String: UInt32] = [
        "rose": 0xE8697F, "task": 0xE8697F,
        "teal": 0x62B8CE, "event": 0x62B8CE,
        "amber": 0xE4AE66, "note": 0xE4AE66,
        "blue": 0x7493E8, "project": 0x7493E8,
        "violet": 0xAE90E6, "purple": 0xAE90E6, "person": 0xAE90E6,
        "green": 0x85BC63, "query": 0x85BC63,
        "coral": 0xFF6B5A, "red": 0xFF6B5A,
        "pink": 0xE093C4,
        "mint": 0x6FC3A8,
        "gold": 0xC9A24B, "yellow": 0xC9A24B,
    ]

    /// FNV-1a 32-bit hash of the lowercased name → palette index. Replicates
    /// the web `paletteIndex` exactly: UTF-16 code units (`charCodeAt`),
    /// wrapping 32-bit multiply (`Math.imul`), and `Math.abs(signed) % len`
    /// (via `Int32.magnitude`) so indices agree across platforms.
    static func index(for name: String) -> Int {
        var h: UInt32 = 2166136261
        for unit in name.lowercased().utf16 {
            h ^= UInt32(unit)
            h = h &* 16777619
        }
        return Int(Int32(bitPattern: h).magnitude) % hues.count
    }

    /// Resolve a `color::` override (a `#rgb`/`#rrggbb` hex or a named hue) to
    /// a packed RGB value, or nil if unrecognized.
    static func resolveOverride(_ raw: String) -> UInt32? {
        let s = raw.trimmingCharacters(in: .whitespaces)
        guard !s.isEmpty else { return nil }
        if s.hasPrefix("#") {
            let hex = String(s.dropFirst())
            if hex.count == 6 { return UInt32(hex, radix: 16) }
            if hex.count == 3 {
                let c = Array(hex)
                return UInt32(String([c[0], c[0], c[1], c[1], c[2], c[2]]), radix: 16)
            }
            return nil
        }
        return named[s.lowercased()]
    }

    /// The color for a tag: the `color::` override when set + valid, else a
    /// deterministic hash of the (cleaned) tag name.
    static func color(for name: String, override: String? = nil) -> Color {
        if let override, let hex = resolveOverride(override) { return Color(hex: hex) }
        return Color(hex: hues[index(for: name)])
    }
}

/// Tag chip — trailing-cluster rendering. Mirrors the web's
/// `.cm-tesela-tag-chip` styling: monospace, low-alpha background tint,
/// per-tag-colored foreground. Split parent/leaf so `#nature/birds`
/// renders as a faded `nature/` followed by the bold `birds`.
struct TagChip: View {
    /// Tag value as it appears in source — accepts `#nature/birds`,
    /// `nature/birds`, or just `birds`.
    let value: String
    /// Optional explicit color override (a `#rrggbb` hex or a named hue) —
    /// the tag page's `color::` frontmatter when threaded. When nil (the
    /// common case) the color is a deterministic hash of the tag name.
    var colorOverride: String? = nil

    @Environment(\.theme) private var theme

    private var cleanName: String {
        value.hasPrefix("#") ? String(value.dropFirst()) : value
    }

    private var parts: (parents: [String], leaf: String) {
        var segments = cleanName.split(separator: "/").map(String.init)
        let leaf = segments.popLast() ?? cleanName
        return (segments, leaf)
    }

    private var tint: Color {
        TagPalette.color(for: cleanName, override: colorOverride)
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
        .background(tint.opacity(0.16))
        .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}

/// Display-only chip for a block's `recurring::` property. Shows a
/// repeat SF Symbol followed by the human-readable recurrence label.
/// Mirrors `TagChip`'s sizing and theming — fgMuted foreground,
/// low-alpha background, monospaced medium font.
struct RecurrenceChip: View {
    let value: String

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 3) {
            Image(systemName: "arrow.triangle.2.circlepath")
                .font(.system(size: 9, weight: .medium))
            Text(RecurrenceFormat.human(value))
        }
        .font(.system(size: 11.5, weight: .medium, design: .monospaced))
        .foregroundStyle(theme.fgMuted)
        .padding(.horizontal, 6)
        .padding(.vertical, 1)
        .background(theme.fgMuted.opacity(0.10))
        .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}

/// Display-only chip for a block's `deadline::` property. Shows a flag
/// SF Symbol followed by the human-readable date label.
/// Mirrors `RecurrenceChip`'s sizing and theming exactly.
struct DeadlineChip: View {
    let value: String

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 3) {
            Image(systemName: "flag.fill")
                .font(.system(size: 9, weight: .medium))
            Text(DateFormat.humanMonthDay(value))
        }
        .font(.system(size: 11.5, weight: .medium, design: .monospaced))
        .foregroundStyle(theme.fgMuted)
        .padding(.horizontal, 6)
        .padding(.vertical, 1)
        .background(theme.fgMuted.opacity(0.10))
        .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}

/// Display-only chip for a block's `scheduled::` property. Shows a calendar
/// SF Symbol followed by the human-readable date label.
/// Mirrors `RecurrenceChip`'s sizing and theming exactly.
struct ScheduledChip: View {
    let value: String

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 3) {
            Image(systemName: "calendar")
                .font(.system(size: 9, weight: .medium))
            Text(DateFormat.humanMonthDay(value))
        }
        .font(.system(size: 11.5, weight: .medium, design: .monospaced))
        .foregroundStyle(theme.fgMuted)
        .padding(.horizontal, 6)
        .padding(.vertical, 1)
        .background(theme.fgMuted.opacity(0.10))
        .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}

/// Display-only chip for an arbitrary block property (`key:: value`) that
/// isn't one of the specially-rendered date/recurrence chips — e.g. a custom
/// `points::` or `testpoints::`. Renders `key value` in the muted chip
/// styling so custom properties are visible on iOS (the web surfaces these
/// via a tag's `display_chips`; iOS shows all non-system props by default).
struct PropertyChip: View {
    let key: String
    let value: String

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 4) {
            Text(key)
                .foregroundStyle(theme.fgFaint)
            Text(value)
                .foregroundStyle(theme.fgMuted)
        }
        .font(.system(size: 11.5, weight: .medium, design: .monospaced))
        .padding(.horizontal, 6)
        .padding(.vertical, 1)
        .background(theme.fgMuted.opacity(0.10))
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
