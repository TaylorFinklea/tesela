import SwiftUI
import UIKit

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

/// Block-level live-presence chip (Phase 3 multi-device): a peer who has a
/// caret in THIS block. Always visible (read AND edit mode), unlike the
/// per-character caret which only exists inside the open `UITextView`. A small
/// colored dot in the peer's color plus its (truncated) device name, in the
/// peer's color over a low-alpha tint — mirrors `TagChip`'s sizing/theming
/// (size 11.5 medium monospaced, 0.16 tint, corner radius 3).
struct RemotePresenceChip: View {
    let name: String
    let color: Color

    var body: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(color)
                .frame(width: 6, height: 6)
            Text(name)
                .lineLimit(1)
        }
        .font(.system(size: 11.5, weight: .medium, design: .monospaced))
        .foregroundStyle(color)
        .padding(.horizontal, 6)
        .padding(.vertical, 1)
        .background(color.opacity(0.16))
        .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}

/// Overflow pill for the presence cluster when more peers share a block than
/// the cluster shows inline (e.g. `+2`). Muted styling, matching the other
/// secondary chips so it reads as "more", not as another peer.
struct RemotePresenceOverflowChip: View {
    let count: Int

    @Environment(\.theme) private var theme

    var body: some View {
        Text("+\(count)")
            .font(.system(size: 11.5, weight: .medium, design: .monospaced))
            .foregroundStyle(theme.fgMuted)
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(theme.fgMuted.opacity(0.10))
            .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}

/// Resolve a Property page's `chip_icon` string into an SF Symbol name (for a
/// known Tabler icon name) OR a raw emoji/text fallback. The iOS port of web
/// `icon-registry.ts::resolveChipIcon` — the Tabler-name → SF Symbol map keeps
/// the same curated set; anything unmatched is treated as raw text (emoji /
/// single char) exactly like web. Exactly one of `symbol`/`emoji` is non-nil
/// when `name` is non-nil.
enum ChipIconRegistry {
    /// Curated Tabler-name → SF Symbol map. Mirrors `TABLER_ICONS` keys so a
    /// property's `chip_icon` resolves to the same glyph family on both
    /// platforms (closest SF Symbol stand-in for each Tabler icon).
    static let sfSymbols: [String: String] = [
        "calendar": "calendar",
        "clock": "clock",
        "flag": "flag.fill",
        "tag": "tag",
        "hourglass": "hourglass",
        "bookmark": "bookmark",
        "hash": "number",
        "link": "link",
        "mail": "envelope",
        "phone": "phone",
        "user": "person",
        "star": "star",
        "repeat": "arrow.triangle.2.circlepath",
        "lock": "lock",
        "checklist": "checklist",
        "checkbox": "checkmark.square",
        "folder": "folder",
        "globe": "globe",
        "bulb": "lightbulb",
        "lightbulb": "lightbulb",
        "sparkles": "sparkles",
    ]

    static func resolve(_ name: String?) -> (symbol: String?, emoji: String?) {
        guard let name, !name.isEmpty else { return (nil, nil) }
        if let symbol = sfSymbols[name.lowercased()] { return (symbol, nil) }
        return (nil, name)
    }
}

/// Pure (UI-free, testable) chip formatting — the iOS port of web
/// `DisplayChip.svelte`'s derivations: effective label mode, effective value
/// format, and the value formatter. Extracted off the `PropertyChip` View so
/// the formatting contract is unit-testable without rendering.
enum ChipFormat {
    /// Effective label mode: explicit `chip_label_mode` > derived (`icon` when
    /// a `chip_icon` is set, else `full`). Mirror web.
    static func labelMode(for def: PropertyDef?) -> ChipLabelMode {
        if let m = def?.chipLabelMode { return m }
        return (def?.chipIcon != nil) ? .icon : .full
    }

    /// Effective value format: explicit `chip_value_format` > type default
    /// (date → month-day, else raw value). Mirror web `defaultValueFormat`.
    static func valueFormat(for def: PropertyDef?) -> ChipValueFormat {
        if let f = def?.chipValueFormat { return f }
        return (def?.valueType == .date) ? .monthDay : .value
    }

    /// Map a select value to a 3-segment bar string by its rank in `choices`
    /// (mirror web `formatBars`). Off-list → a single filled segment.
    static func bars(_ v: String, choices: [String]) -> String {
        let target = v.trimmingCharacters(in: .whitespaces).lowercased()
        let idx = choices.firstIndex { $0.lowercased() == target }
        let total = max(choices.count, 1)
        let rank = (idx == nil) ? 1 : idx! + 1
        let filled = max(1, Int((Double(rank) / Double(total) * 3).rounded()))
        return String(repeating: "▰", count: filled) + String(repeating: "▱", count: 3 - filled)
    }

    static func truncate(_ v: String, max: Int) -> String {
        v.count > max ? String(v.prefix(max - 1)) + "…" : v
    }

    /// The display value after applying the effective `valueFormat` for `def`
    /// (mirror web `formattedValue`). `recurring` is handled by its own chip.
    static func formattedValue(_ value: String, def: PropertyDef?) -> String {
        let v = value.trimmingCharacters(in: .whitespaces)
        switch valueFormat(for: def) {
        case .monthDay:
            return DateFormat.humanMonthDay(v)
        case .iso:
            return v.replacingOccurrences(of: #"^\[\[|\]\]$"#, with: "", options: .regularExpression)
        case .bars:
            guard let def, def.valueType == .select || def.valueType == .multiSelect else {
                return truncate(v, max: 24)
            }
            return bars(v, choices: def.choices)
        case .truncate:
            return truncate(v, max: 10)
        case .value:
            return truncate(v, max: 24)
        }
    }

    /// The label text for `full`/`short` modes (`icon`/`none` → nil). Mirror
    /// web `labelText`. `fallbackKey` is used when the def is absent.
    static func labelText(for def: PropertyDef?, fallbackKey: String) -> String? {
        switch labelMode(for: def) {
        case .none, .icon: return nil
        case .short: return def?.chipShortLabel ?? String((def?.name ?? fallbackKey).prefix(4))
        case .full: return def?.name ?? fallbackKey
        }
    }
}

/// Display-only chip for an arbitrary block property (`key:: value`) that
/// isn't one of the specially-rendered date/recurrence chips — e.g. a custom
/// `points::` or `testpoints::`. Renders the property label + formatted value
/// in the muted chip styling so custom properties are visible on iOS (the web
/// surfaces these via a tag's `display_chips`; iOS shows all non-system props
/// by default). Visualization (icon / label mode / value format) is driven by
/// the resolved `PropertyDef` so a property looks the same wherever it surfaces
/// — the iOS port of web `DisplayChip.svelte`.
struct PropertyChip: View {
    let key: String
    let value: String
    /// The resolved property def (off the registry) that drives the chip's
    /// label mode, value format, and icon. `nil` → fall back to the raw
    /// `key value` rendering (legacy behaviour for properties with no def).
    var def: PropertyDef? = nil
    /// Phase 5.6: per-choice `choice_colors` tint for a select/multi-select
    /// VALUE chip, resolved off the registry. `nil` → the default muted
    /// chip (uncolored choices look unchanged). Mirrors web `DisplayChip`'s
    /// tinted recipe (translucent background + saturated foreground); the
    /// task STATUS marker is NOT routed here (it stays priority-colored).
    var tint: Color? = nil

    @Environment(\.theme) private var theme

    // Web parity (DisplayChip): the property KEY stays muted; only the VALUE
    // text carries the choice color, blended ~22% toward the theme foreground
    // so a saturated choice (bright green/amber) stays readable in both themes
    // (SwiftUI has no color-mix on our target — see Color.mixed below).
    private var keyColor: Color { theme.fgFaint }
    private var valueColor: Color { tint?.mixed(toward: theme.fgDefault, 0.22) ?? theme.fgMuted }
    private var bgColor: Color { (tint ?? theme.fgMuted).opacity(tint == nil ? 0.10 : 0.16) }

    private var labelMode: ChipLabelMode { ChipFormat.labelMode(for: def) }
    private var formattedValue: String { ChipFormat.formattedValue(value, def: def) }
    private var labelText: String? { ChipFormat.labelText(for: def, fallbackKey: key) }
    private var icon: (symbol: String?, emoji: String?) {
        ChipIconRegistry.resolve(def?.chipIcon)
    }

    var body: some View {
        HStack(spacing: 4) {
            if labelMode == .icon, icon.symbol != nil || icon.emoji != nil {
                if let symbol = icon.symbol {
                    Image(systemName: symbol)
                        .font(.system(size: 9, weight: .medium))
                        .foregroundStyle(keyColor)
                } else if let emoji = icon.emoji {
                    Text(emoji).foregroundStyle(keyColor)
                }
            } else if let labelText {
                Text(labelText)
                    .foregroundStyle(keyColor)
            }
            Text(formattedValue)
                .foregroundStyle(valueColor)
        }
        .font(.system(size: 11.5, weight: .medium, design: .monospaced))
        .padding(.horizontal, 6)
        .padding(.vertical, 1)
        .background(bgColor)
        .overlay(
            RoundedRectangle(cornerRadius: 3)
                .strokeBorder((tint ?? .clear).opacity(tint == nil ? 0 : 0.32), lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}

/// Blend one SwiftUI `Color` toward another by `fraction` (0 = self,
/// 1 = other) via UIColor components — the iOS stand-in for web's CSS
/// `color-mix`, used by `PropertyChip` to keep tinted value text readable.
private extension Color {
    func mixed(toward other: Color, _ fraction: Double) -> Color {
        var ra: CGFloat = 0, ga: CGFloat = 0, ba: CGFloat = 0, aa: CGFloat = 0
        var rb: CGFloat = 0, gb: CGFloat = 0, bb: CGFloat = 0, ab: CGFloat = 0
        guard UIColor(self).getRed(&ra, green: &ga, blue: &ba, alpha: &aa),
              UIColor(other).getRed(&rb, green: &gb, blue: &bb, alpha: &ab) else { return self }
        let f = CGFloat(fraction)
        return Color(red: Double(ra + (rb - ra) * f),
                     green: Double(ga + (gb - ga) * f),
                     blue: Double(ba + (bb - ba) * f))
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
