import Foundation

// MARK: - AnyJSONDict (Decodable [String: Any])

/// Decodes an arbitrary JSON object into `[String: Any]` — used to re-add the
/// `metadata.custom` map (the server serializes it as a JSON object) to the
/// `.http` note decoder without hand-rolling a struct per nested shape. Arrays,
/// nested objects, strings, bools, and numbers are preserved; nulls drop.
struct AnyJSONDict: Decodable {
    let value: [String: Any]

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: AnyCodingKey.self)
        var out: [String: Any] = [:]
        for key in container.allKeys {
            if let v = try AnyJSONDict.decode(container: container, key: key) {
                out[key.stringValue] = v
            }
        }
        value = out
    }

    private static func decode(container: KeyedDecodingContainer<AnyCodingKey>, key: AnyCodingKey) throws -> Any? {
        if let s = try? container.decode(String.self, forKey: key) { return s }
        if let b = try? container.decode(Bool.self, forKey: key) { return b }
        if let i = try? container.decode(Int.self, forKey: key) { return i }
        if let d = try? container.decode(Double.self, forKey: key) { return d }
        if let nested = try? container.decode(AnyJSONDict.self, forKey: key) { return nested.value }
        if var arr = try? container.nestedUnkeyedContainer(forKey: key) {
            return try decodeArray(&arr)
        }
        return nil
    }

    private static func decodeArray(_ container: inout UnkeyedDecodingContainer) throws -> [Any] {
        var out: [Any] = []
        while !container.isAtEnd {
            if let s = try? container.decode(String.self) { out.append(s); continue }
            if let b = try? container.decode(Bool.self) { out.append(b); continue }
            if let i = try? container.decode(Int.self) { out.append(i); continue }
            if let d = try? container.decode(Double.self) { out.append(d); continue }
            if let nested = try? container.decode(AnyJSONDict.self) { out.append(nested.value); continue }
            if var sub = try? container.nestedUnkeyedContainer() {
                out.append(try decodeArray(&sub)); continue
            }
            _ = try? container.superDecoder() // skip nulls / unknowns
        }
        return out
    }
}

private struct AnyCodingKey: CodingKey {
    var stringValue: String
    var intValue: Int?
    init?(stringValue: String) { self.stringValue = stringValue; self.intValue = nil }
    init?(intValue: Int) { self.stringValue = String(intValue); self.intValue = intValue }
}

/// iOS port of the web property/type registry (`web/src/lib/property-registry.ts`).
///
/// The registry is built CLIENT-SIDE from synced Property/Tag pages (notes whose
/// frontmatter declares `type: "Property"` / `type: "Tag"`) — NOT from `GET /types`.
/// The rich metadata (`nl_triggers`, `choice_colors`, `chip_*`, chord keys,
/// `property_overrides`) lives ONLY in Property/Tag-page frontmatter, never in the
/// DB/API surface, so iOS must parse the pages exactly like web does.
///
/// Read layer only (Phase 5.2): resolution semantics mirror web
/// `getTagPropertyDefs` + `applyOverride` (which mirror Rust `apply_override`).
/// No editor/toolbar/chip UI consumes this yet — P5.3-5.6 wire it up.

// MARK: - Value-type / enums (mirror property-registry.ts:8-93)

/// The declared value-type of a Property page (`value_type:` frontmatter).
enum PropertyType: String, Equatable {
    case text
    case number
    case select
    case multiSelect = "multi-select"
    case date
    case checkbox
    case url
    case email
    case phone
    case object

    /// Coerce an arbitrary frontmatter string to a known type, defaulting to
    /// `.text` (mirror of `(c.value_type as PropertyType) || "text"`).
    static func parse(_ raw: String?) -> PropertyType {
        guard let raw, let t = PropertyType(rawValue: raw) else { return .text }
        return t
    }
}

let PROPERTY_TYPE_LABELS: [PropertyType: String] = [
    .text: "Text",
    .number: "Number",
    .select: "Select",
    .multiSelect: "Multi-select",
    .date: "Date",
    .checkbox: "Checkbox",
    .url: "URL",
    .email: "Email",
    .phone: "Phone",
    .object: "Object",
]

/// Per-type visibility (Anytype/Logseq-DB 3-state). Serializes as the same
/// `on_new`/`on_set`/`hidden` strings used in `property_overrides.{Prop}.show`.
enum Visibility: String, Equatable {
    case onNew = "on_new"
    case onSet = "on_set"
    case hidden

    static func parse(_ raw: String?) -> Visibility? {
        guard let raw else { return nil }
        return Visibility(rawValue: raw)
    }
}

/// How to label a property in chip form (`chip_label_mode:`).
enum ChipLabelMode: String, Equatable {
    case full
    case short
    case icon
    case none
}

/// How to render a chip value (`chip_value_format:`).
enum ChipValueFormat: String, Equatable {
    case value
    case monthDay = "month-day"
    case iso
    case bars
    case truncate
}

// MARK: - PropertyDef (mirror property-registry.ts:42-93)

/// A resolved property definition. Mirrors the web `PropertyDefinition`.
/// A bare def off `buildRegistry` carries `show == nil`; the per-type resolver
/// (`resolvedType(forTag:)`) always sets a concrete `show`.
struct PropertyDef: Equatable {
    var name: String
    var valueType: PropertyType
    var choices: [String]
    var def: String?
    var show: Visibility?
    var hideByDefault: Bool
    var hideEmpty: Bool
    var chipIcon: String?
    var chipLabelMode: ChipLabelMode?
    var chipShortLabel: String?
    var chipValueFormat: ChipValueFormat?
    var chordKey: String?
    var valueChordKeys: [String: String]
    var choiceColors: [String: String]
    var nlTriggers: [String]
}

// MARK: - TypeDef (resolved Tag-page header info)

/// A resolved type definition for a Tag (mirror of `TypeDefinition` —
/// the `icon`/`plural` come straight off the Tag page's `custom`).
struct TypeDef: Equatable {
    var name: String
    var plural: String
    var icon: String?
    var properties: [PropertyDef]
}

// MARK: - Frontmatter parsing (general nested-YAML → [String: Any])

/// Parses YAML frontmatter into a `[String: Any]` map, handling the NESTED
/// shapes the single-line scrapers (`parseNoteTypeFromFrontmatter`,
/// `parseTagsFromFrontmatter`) can't:
///   - flow arrays:  `nl_triggers: [a, b]`, `choices: ["x", "y"]`
///   - flow maps:    `value_chord_keys: {b: backlog}`
///   - compact JSON: `property_overrides: {"Status": {"choices": [...], "show": "on_new"}}`
///                   (the web writes overrides as inline JSON, which is valid flow YAML)
/// plus block-style maps/sequences. Values are coerced to `String`, `Bool`,
/// `[Any]`, or `[String: Any]`. This generalizes the existing scrapers — a
/// single-line `type: "Property"` still reads as the string `"Property"`.
enum FrontmatterParser {

    /// Extract the raw frontmatter body (between the opening `---\n` and the
    /// closing `\n---`) from a note's full content. Returns `""` when there is
    /// no fenced frontmatter.
    static func body(from content: String) -> String {
        guard content.hasPrefix("---") else { return "" }
        let afterOpen = content.index(content.startIndex, offsetBy: 3)
        guard let close = content.range(of: "\n---", options: [], range: afterOpen..<content.endIndex) else {
            return ""
        }
        // Body is between the opening fence's newline and the closing fence.
        let start = content.range(of: "\n", range: afterOpen..<content.endIndex)?.upperBound ?? afterOpen
        return String(content[start..<close.lowerBound])
    }

    /// Parse a note's full content's frontmatter into `[String: Any]`.
    static func parse(content: String) -> [String: Any] {
        return parseBody(body(from: content))
    }

    /// Parse a frontmatter body (no fences) into `[String: Any]`.
    static func parseBody(_ fm: String) -> [String: Any] {
        var result: [String: Any] = [:]
        let lines = fm.components(separatedBy: "\n")
        var i = 0
        while i < lines.count {
            let rawLine = lines[i]
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            // Skip blanks and comments.
            if line.isEmpty || line.hasPrefix("#") { i += 1; continue }
            // A top-level key is at indent 0.
            guard indent(of: rawLine) == 0, let colon = topLevelColon(in: line) else {
                i += 1
                continue
            }
            let key = String(line[line.startIndex..<colon]).trimmingCharacters(in: .whitespaces)
            let valuePart = String(line[line.index(after: colon)...]).trimmingCharacters(in: .whitespaces)
            if !valuePart.isEmpty {
                // Inline value (scalar, flow array, or flow map / compact JSON).
                result[key] = parseScalarOrFlow(valuePart)
                i += 1
            } else {
                // Block-style: the value is the indented region that follows.
                let (val, next) = parseBlock(lines: lines, start: i + 1, parentIndent: 0)
                if let val { result[key] = val }
                i = next
            }
        }
        return result
    }

    // MARK: block-style nested parsing

    /// Parse the indented region beginning at `start` whose entries are more
    /// indented than `parentIndent`. Returns the parsed value (a `[String: Any]`
    /// map, an `[Any]` sequence, or nil when the region is empty) and the index
    /// of the first line NOT consumed.
    private static func parseBlock(lines: [String], start: Int, parentIndent: Int) -> (Any?, Int) {
        // Find the indent of the first non-blank line in the region.
        var i = start
        var childIndent: Int? = nil
        while i < lines.count {
            let l = lines[i].trimmingCharacters(in: .whitespaces)
            if l.isEmpty || l.hasPrefix("#") { i += 1; continue }
            let ind = indent(of: lines[i])
            if ind <= parentIndent { return (nil, start) } // region is empty
            childIndent = ind
            break
        }
        guard let childIndent else { return (nil, i) }

        let isSequence = lines[i].trimmingCharacters(in: .whitespaces).hasPrefix("- ")
            || lines[i].trimmingCharacters(in: .whitespaces) == "-"
        if isSequence {
            var seq: [Any] = []
            var j = i
            while j < lines.count {
                let raw = lines[j]
                let trimmed = raw.trimmingCharacters(in: .whitespaces)
                if trimmed.isEmpty || trimmed.hasPrefix("#") { j += 1; continue }
                let ind = indent(of: raw)
                if ind < childIndent { break }
                if ind > childIndent { j += 1; continue } // defensive; nested seqs uncommon here
                guard trimmed.hasPrefix("-") else { break }
                let item = trimmed.dropFirst().trimmingCharacters(in: .whitespaces)
                if item.isEmpty {
                    seq.append("")
                } else {
                    seq.append(parseScalarOrFlow(String(item)))
                }
                j += 1
            }
            return (seq, j)
        } else {
            var map: [String: Any] = [:]
            var j = i
            while j < lines.count {
                let raw = lines[j]
                let trimmed = raw.trimmingCharacters(in: .whitespaces)
                if trimmed.isEmpty || trimmed.hasPrefix("#") { j += 1; continue }
                let ind = indent(of: raw)
                if ind < childIndent { break }
                if ind > childIndent { j += 1; continue }
                guard let colon = topLevelColon(in: trimmed) else { j += 1; continue }
                let k = String(trimmed[trimmed.startIndex..<colon]).trimmingCharacters(in: .whitespaces)
                let v = String(trimmed[trimmed.index(after: colon)...]).trimmingCharacters(in: .whitespaces)
                if !v.isEmpty {
                    map[unquote(k)] = parseScalarOrFlow(v)
                    j += 1
                } else {
                    let (nested, next) = parseBlock(lines: lines, start: j + 1, parentIndent: childIndent)
                    if let nested { map[unquote(k)] = nested }
                    j = next
                }
            }
            return (map, j)
        }
    }

    // MARK: scalar / flow parsing

    /// Parse a single inline value: a flow array `[...]`, a flow map / compact
    /// JSON `{...}`, a quoted string, a bool, or a bare scalar string.
    static func parseScalarOrFlow(_ raw: String) -> Any {
        let s = stripTrailingComment(raw).trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("[") && s.hasSuffix("]") {
            return parseFlowSeq(String(s.dropFirst().dropLast()))
        }
        if s.hasPrefix("{") && s.hasSuffix("}") {
            return parseFlowMap(String(s.dropFirst().dropLast()))
        }
        return parseScalar(s)
    }

    /// Parse a bare scalar: unquote, then coerce `true`/`false` to `Bool`.
    /// Numbers stay as `String` (the web registry reads everything as the raw
    /// value type it needs; numeric coercion isn't required by any registry
    /// consumer — choices/defaults/triggers are all strings).
    private static func parseScalar(_ raw: String) -> Any {
        let s = raw.trimmingCharacters(in: .whitespaces)
        if (s.hasPrefix("\"") && s.hasSuffix("\"") && s.count >= 2)
            || (s.hasPrefix("'") && s.hasSuffix("'") && s.count >= 2) {
            return String(s.dropFirst().dropLast())
        }
        if s == "true" { return true }
        if s == "false" { return false }
        return s
    }

    /// Parse the inside of a flow sequence (`a, b, "c"` → `[Any]`), honoring
    /// nested `[...]`/`{...}` and quoted commas.
    private static func parseFlowSeq(_ inner: String) -> [Any] {
        return splitTopLevel(inner, by: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
            .map { parseScalarOrFlow($0) }
    }

    /// Parse the inside of a flow map (`a: b, "c": {..}` → `[String: Any]`),
    /// honoring nested structures + quoted keys/values.
    private static func parseFlowMap(_ inner: String) -> [String: Any] {
        var map: [String: Any] = [:]
        for pair in splitTopLevel(inner, by: ",") {
            let p = pair.trimmingCharacters(in: .whitespaces)
            if p.isEmpty { continue }
            guard let colon = topLevelColon(in: p) else { continue }
            let k = unquote(String(p[p.startIndex..<colon]).trimmingCharacters(in: .whitespaces))
            let v = String(p[p.index(after: colon)...]).trimmingCharacters(in: .whitespaces)
            map[k] = parseScalarOrFlow(v)
        }
        return map
    }

    // MARK: lexical helpers

    /// Split `s` on `sep` at the TOP nesting level only — commas/colons inside
    /// `[...]`, `{...}`, or quotes don't split. Essential for compact-JSON
    /// overrides like `{"Status": {"choices": ["a","b"]}}`.
    private static func splitTopLevel(_ s: String, by sep: Character) -> [String] {
        var parts: [String] = []
        var depth = 0
        var inQuote: Character? = nil
        var current = ""
        for ch in s {
            if let q = inQuote {
                current.append(ch)
                if ch == q { inQuote = nil }
                continue
            }
            switch ch {
            case "\"", "'":
                inQuote = ch
                current.append(ch)
            case "[", "{":
                depth += 1
                current.append(ch)
            case "]", "}":
                depth -= 1
                current.append(ch)
            case sep where depth == 0:
                parts.append(current)
                current = ""
            default:
                current.append(ch)
            }
        }
        if !current.trimmingCharacters(in: .whitespaces).isEmpty || !parts.isEmpty {
            parts.append(current)
        }
        return parts
    }

    /// Index of the first `:` at the top nesting level (not inside quotes /
    /// brackets), or nil. Used to split `key: value` without tripping on a
    /// colon inside an inline value.
    private static func topLevelColon(in s: String) -> String.Index? {
        var depth = 0
        var inQuote: Character? = nil
        var idx = s.startIndex
        while idx < s.endIndex {
            let ch = s[idx]
            if let q = inQuote {
                if ch == q { inQuote = nil }
            } else {
                switch ch {
                case "\"", "'": inQuote = ch
                case "[", "{": depth += 1
                case "]", "}": depth -= 1
                case ":" where depth == 0: return idx
                default: break
                }
            }
            idx = s.index(after: idx)
        }
        return nil
    }

    /// Strip a trailing ` # comment` outside quotes/brackets. Conservative —
    /// only fires when a `#` is preceded by whitespace at the top level.
    private static func stripTrailingComment(_ s: String) -> String {
        var depth = 0
        var inQuote: Character? = nil
        var prev: Character? = nil
        var idx = s.startIndex
        while idx < s.endIndex {
            let ch = s[idx]
            if let q = inQuote {
                if ch == q { inQuote = nil }
            } else {
                switch ch {
                case "\"", "'": inQuote = ch
                case "[", "{": depth += 1
                case "]", "}": depth -= 1
                case "#" where depth == 0 && (prev == " " || prev == "\t"):
                    return String(s[s.startIndex..<idx])
                default: break
                }
            }
            prev = ch
            idx = s.index(after: idx)
        }
        return s
    }

    private static func unquote(_ s: String) -> String {
        if (s.hasPrefix("\"") && s.hasSuffix("\"") && s.count >= 2)
            || (s.hasPrefix("'") && s.hasSuffix("'") && s.count >= 2) {
            return String(s.dropFirst().dropLast())
        }
        return s
    }

    /// Number of leading spaces (tabs count as one) on a raw line.
    private static func indent(of line: String) -> Int {
        var n = 0
        for ch in line {
            if ch == " " || ch == "\t" { n += 1 } else { break }
        }
        return n
    }
}

// MARK: - RegistryNote (minimal input shape)

/// The minimal note shape the registry reads — mirrors what the web registry
/// touches on a `Note` (`title`, `metadata.note_type`, `metadata.custom`).
/// Decoupled from `APINote` so the resolver is pure + fixture-testable.
struct RegistryNote {
    var title: String
    var noteType: String?
    var custom: [String: Any]

    init(title: String, noteType: String?, custom: [String: Any]) {
        self.title = title
        self.noteType = noteType
        self.custom = custom
    }

    /// Build from a note's raw markdown content (the ONE local-parse path —
    /// `.relay` local files, `.http` snapshotted files, all flow through here).
    init(title: String, noteType: String?, content: String) {
        self.title = title
        self.noteType = noteType
        self.custom = FrontmatterParser.parse(content: content)
    }
}

// MARK: - PropOverride (mirror property-registry.ts:229-261)

/// A resolved per-type property override. `choices == nil` means "no choices
/// override"; `hideChoices` defaults to `[]`.
struct PropOverride: Equatable {
    var choices: [String]?
    var def: String?
    var show: Visibility?
    var hideChoices: [String]
}

// MARK: - Registry

/// The built property/type registry. Mirrors web `buildRegistry` +
/// `buildInheritanceMap` + `getTagPropertyDefs`.
struct PropertyRegistry {
    /// `lower(propertyName) → def` from Property pages.
    private(set) var properties: [String: PropertyDef] = [:]
    /// `lower(tagName) → lower(parentTagName)` from Tag pages' `extends:`.
    private(set) var inheritance: [String: String] = [:]
    /// All Tag/Property notes retained for the membership + override passes.
    private var notes: [RegistryNote] = []

    init() {}

    // MARK: build

    /// Build the registry from a set of notes (mirror of `buildRegistry` +
    /// `buildInheritanceMap`).
    static func build(from notes: [RegistryNote]) -> PropertyRegistry {
        var reg = PropertyRegistry()
        reg.notes = notes
        for n in notes {
            if let def = parsePropertyPage(n) {
                reg.properties[def.name.lowercased()] = def
            }
            if n.noteType == "Tag" {
                if let ext = (n.custom["extends"] as? String)?.trimmingCharacters(in: .whitespaces),
                   !ext.isEmpty {
                    reg.inheritance[n.title.lowercased()] = ext.lowercased()
                }
            }
        }
        return reg
    }

    // MARK: parsePropertyPage (mirror :97-153)

    static func parsePropertyPage(_ note: RegistryNote) -> PropertyDef? {
        guard note.noteType == "Property" else { return nil }
        let c = note.custom

        let labelMode = (c["chip_label_mode"] as? String).flatMap(ChipLabelMode.init(rawValue:))
        let valueFormat = (c["chip_value_format"] as? String).flatMap(ChipValueFormat.init(rawValue:))

        // chord_key: coerce to the first letter, lowercased.
        let chordKeyRaw = c["chord_key"] as? String
        let chordKey: String? = {
            guard let r = chordKeyRaw, let first = r.first else { return nil }
            return String(first).lowercased()
        }()

        // value_chord_keys: { choice: letter } — single lowercase letter each.
        var valueChordKeys: [String: String] = [:]
        if let vck = c["value_chord_keys"] as? [String: Any] {
            for (k, v) in vck {
                guard let s = v as? String, let first = s.first else { continue }
                let ch = String(first).lowercased()
                if ch.range(of: "[a-z]", options: .regularExpression) != nil {
                    valueChordKeys[k.lowercased()] = ch
                }
            }
        }

        // choice_colors: { choice: cssColor } — keys lowercased, values verbatim.
        var choiceColors: [String: String] = [:]
        if let cc = c["choice_colors"] as? [String: Any] {
            for (k, v) in cc {
                if let s = v as? String, !s.trimmingCharacters(in: .whitespaces).isEmpty {
                    choiceColors[k.lowercased()] = s.trimmingCharacters(in: .whitespaces)
                }
            }
        }

        let nlTriggers: [String] = (c["nl_triggers"] as? [Any])?
            .compactMap { $0 as? String }
            .map { $0.lowercased() } ?? []

        return PropertyDef(
            name: note.title,
            valueType: PropertyType.parse(c["value_type"] as? String),
            choices: (c["choices"] as? [Any])?.compactMap { $0 as? String } ?? [],
            def: c["default"] as? String,
            show: nil,
            hideByDefault: (c["hide_by_default"] as? Bool) == true,
            hideEmpty: (c["hide_empty"] as? Bool) != false, // default true
            chipIcon: c["chip_icon"] as? String,
            chipLabelMode: labelMode,
            chipShortLabel: c["chip_short_label"] as? String,
            chipValueFormat: valueFormat,
            chordKey: chordKey,
            valueChordKeys: valueChordKeys,
            choiceColors: choiceColors,
            nlTriggers: nlTriggers
        )
    }

    // MARK: inheritance chain (mirror :210-220)

    /// Full ancestor chain for a tag, starting with itself. Cycle-safe (max 10).
    func tagChain(_ tagName: String) -> [String] {
        var chain = [tagName.lowercased()]
        var current = tagName.lowercased()
        for _ in 0..<10 {
            guard let parent = inheritance[current], !chain.contains(parent) else { break }
            chain.append(parent)
            current = parent
        }
        return chain
    }

    // MARK: parseHiddenChoices (mirror :184-192)

    /// Parse legacy `hidden_{Prop}: ["v1","v2"]` keys off a Tag page's custom.
    static func parseHiddenChoices(_ custom: [String: Any]) -> [String: [String]] {
        var result: [String: [String]] = [:]
        for (key, val) in custom {
            if key.hasPrefix("hidden_"), let arr = (val as? [Any])?.compactMap({ $0 as? String }) {
                result[String(key.dropFirst("hidden_".count))] = arr
            }
        }
        return result
    }

    // MARK: parsePropOverride (mirror :243-261)

    /// Parse one `property_overrides.{Prop}` entry. Malformed → empty override.
    static func parsePropOverride(_ v: Any?) -> PropOverride {
        let empty = PropOverride(choices: nil, def: nil, show: nil, hideChoices: [])
        guard let obj = v as? [String: Any] else { return empty }
        func strArray(_ val: Any?) -> [String] {
            (val as? [Any])?.compactMap { $0 as? String } ?? []
        }
        let show = Visibility.parse(obj["show"] as? String)
        return PropOverride(
            // `choices` present (any type) → coerced to a string array; a
            // present-but-non-array becomes `[]` (mirror of TS str_array).
            choices: obj["choices"] != nil ? strArray(obj["choices"]) : nil,
            def: obj["default"] as? String,
            show: show,
            hideChoices: strArray(obj["hide_choices"])
        )
    }

    // MARK: buildOverrides (mirror :280-307)

    /// Build the resolved override map for a tag, walking rows child→parent.
    /// `property_overrides.{Prop}` is FIRST-INSERT-WINS; the legacy
    /// `hidden_{Prop}` fold is ADDITIVE into `hide_choices`.
    static func buildOverrides(
        rows: [(overrides: [String: Any], hidden: [String: [String]])]
    ) -> [String: PropOverride] {
        var map: [String: PropOverride] = [:]
        for row in rows {
            // property_overrides.{Prop} — first-insert-wins (child rows first).
            for (prop, val) in row.overrides {
                let key = prop.lowercased()
                if map[key] == nil {
                    map[key] = parsePropOverride(val)
                }
            }
            // Legacy hidden_{Prop}: additive subtract regardless of first-insert.
            for (prop, vals) in row.hidden {
                let key = prop.lowercased()
                var entry = map[key] ?? PropOverride(choices: nil, def: nil, show: nil, hideChoices: [])
                for h in vals where !entry.hideChoices.contains(h) {
                    entry.hideChoices.append(h)
                }
                map[key] = entry
            }
        }
        return map
    }

    // MARK: applyOverride (mirror :321-343)

    /// Apply a resolved override to a single def. `show` is ALWAYS set here.
    /// Precedence: (a) choices REPLACE if override has them; (b) SUBTRACT
    /// hide_choices; (c) default override wins; (d) show override else derive.
    static func applyOverride(_ def: PropertyDef, _ over: PropOverride?, hideByDefault: Bool) -> PropertyDef {
        var choices = def.choices
        if let over {
            if let oc = over.choices {
                choices = oc
            }
            if !over.hideChoices.isEmpty {
                let hidden = Set(over.hideChoices)
                choices = choices.filter { !hidden.contains($0) }
            }
        }
        let show: Visibility = over?.show ?? (hideByDefault ? .hidden : .onNew)
        var out = def
        out.choices = choices
        out.def = over?.def ?? def.def
        out.show = show
        return out
    }

    // MARK: getTagPropertyDefs (mirror :356-429)

    /// Resolve a tag's full property-def list — chain walk + override merge +
    /// per-property `applyOverride`. Deduped by lowercased property name.
    func resolvedDefs(forTag tagName: String) -> [PropertyDef] {
        let chain = tagChain(tagName)

        // SEPARATE override pass: walk rows child→parent, first-insert-wins.
        var overrideRows: [(overrides: [String: Any], hidden: [String: [String]])] = []
        for tag in chain {
            guard let tagPage = notes.first(where: {
                $0.title.lowercased() == tag && $0.noteType == "Tag"
            }) else { continue }
            let c = tagPage.custom
            let rawOverrides = (c["property_overrides"] as? [String: Any]) ?? [:]
            overrideRows.append((overrides: rawOverrides, hidden: Self.parseHiddenChoices(c)))
        }
        let overrides = Self.buildOverrides(rows: overrideRows)

        // Membership pass: union along the chain, deduped child-first.
        var seen = Set<String>()
        var out: [PropertyDef] = []
        for tag in chain {
            guard let tagPage = notes.first(where: {
                $0.title.lowercased() == tag && $0.noteType == "Tag"
            }) else { continue }
            guard let tagProps = (tagPage.custom["tag_properties"] as? [Any])?.compactMap({ $0 as? String }) else {
                continue
            }
            for propName in tagProps {
                let key = propName.lowercased()
                if seen.contains(key) { continue }
                seen.insert(key)
                let over = overrides[key]
                if let def = properties[key] {
                    out.append(Self.applyOverride(def, over, hideByDefault: def.hideByDefault))
                } else {
                    // No global Property page — a text-stub def still receives
                    // the override (mirror §3.5c).
                    let stub = PropertyDef(
                        name: propName,
                        valueType: .text,
                        choices: [],
                        def: nil,
                        show: nil,
                        hideByDefault: false,
                        hideEmpty: true,
                        chipIcon: nil,
                        chipLabelMode: nil,
                        chipShortLabel: nil,
                        chipValueFormat: nil,
                        chordKey: nil,
                        valueChordKeys: [:],
                        choiceColors: [:],
                        nlTriggers: []
                    )
                    out.append(Self.applyOverride(stub, over, hideByDefault: false))
                }
            }
        }
        return out
    }

    // MARK: resolvedType (header + properties)

    /// The fully-resolved type for a tag: header `plural`/`icon` (off the tag's
    /// OWN page) plus the resolved property defs. `plural` falls back to the
    /// tag name when no `plural:` frontmatter is declared (mirror of
    /// `TagPageRenderer` :70-82).
    func resolvedType(forTag tagName: String) -> TypeDef {
        let ownPage = notes.first(where: {
            $0.title.lowercased() == tagName.lowercased() && $0.noteType == "Tag"
        })
        let plural: String = {
            if let p = (ownPage?.custom["plural"] as? String)?.trimmingCharacters(in: .whitespaces), !p.isEmpty {
                return p
            }
            return ownPage?.title ?? tagName
        }()
        let icon = ownPage?.custom["icon"] as? String
        return TypeDef(
            name: ownPage?.title ?? tagName,
            plural: plural,
            icon: icon,
            properties: resolvedDefs(forTag: tagName)
        )
    }

    // MARK: type names (capture type picker)

    /// All Tag-page type names (e.g. Task, Project) a captured block can be
    /// tagged as — every `type: Tag` page except the abstract "Root Tag" base
    /// — de-duplicated (case-insensitive) and sorted. Drives the Capture
    /// composer's type picker. Empty when no Tag pages are known yet; the
    /// caller falls back to `buildBuiltins()` so Task/Project are always
    /// offerable on a not-yet-synced registry.
    func typeNames() -> [String] {
        var seen = Set<String>()
        var names: [String] = []
        for n in notes where n.noteType == "Tag" {
            let title = n.title.trimmingCharacters(in: .whitespaces)
            guard !title.isEmpty, title.lowercased() != "root tag" else { continue }
            if seen.insert(title.lowercased()).inserted { names.append(title) }
        }
        return names.sorted()
    }

    /// Whether this registry can lift ANY inline-NLP token for `tagName` — i.e.
    /// the type resolves at least one property that declares an `nl_trigger`.
    /// The capture path uses this to decide whether the live registry actually
    /// carries the type's NLP config or must fall back to the built-ins: the
    /// type picker offers Task/Project even before their Property pages have
    /// synced (it falls back to `buildBuiltins()`), so resolving NLP against an
    /// empty / partially-synced registry would find no triggers — the block
    /// gets tagged but nothing is stripped. `false` here means "fall back".
    func hasLiftableDefs(forTag tagName: String) -> Bool {
        resolvedDefs(forTag: tagName).contains { !$0.nlTriggers.isEmpty }
    }

    // MARK: built-ins (mock backend)

    /// The canonical built-in Property/Tag pages — mirrors the server seed
    /// (`tesela-server/src/lib.rs:216-224`). Used to seed the registry in
    /// `.mock` mode, where there is no synced sandbox to parse from. Parsed
    /// through the same `FrontmatterParser` path the real backends use so the
    /// built-in shapes exercise the same code.
    static let builtinPages: [String] = [
        "---\ntitle: \"Root Tag\"\ntype: \"Tag\"\nicon: \"📄\"\ntag_properties: []\ntags: []\n---\n",
        "---\ntitle: \"Task\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"checkbox\"\nplural: \"Tasks\"\ntag_properties: [\"Status\", \"Priority\", \"Deadline\", \"Scheduled\", \"Points\"]\nproperty_overrides: {Status: {choices: [todo, doing, done, blocked], show: on_new, default: todo}}\ntags: []\n---\n",
        "---\ntitle: \"Project\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"folder\"\nplural: \"Projects\"\ntag_properties: [\"Status\", \"Deadline\"]\nproperty_overrides: {Status: {choices: [planned, active, shipped]}}\ntags: []\n---\n",
        "---\ntitle: \"Status\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"backlog\", \"todo\", \"doing\", \"in-review\", \"done\", \"canceled\"]\ndefault: \"todo\"\ntags: []\n---\n",
        "---\ntitle: \"Priority\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"p1\", \"p2\", \"p3\", \"p4\"]\ndefault: \"p4\"\nnl_triggers: [\"p1\", \"p2\", \"p3\", \"p4\"]\ntags: []\n---\n",
        "---\ntitle: \"Deadline\"\ntype: \"Property\"\nvalue_type: \"date\"\nnl_triggers: [\"due\", \"deadline\"]\ntags: []\n---\n",
        "---\ntitle: \"Scheduled\"\ntype: \"Property\"\nvalue_type: \"date\"\nnl_triggers: [\"scheduled\"]\ntags: []\n---\n",
        "---\ntitle: \"Points\"\ntype: \"Property\"\nvalue_type: \"number\"\nnl_triggers: [\"points\", \"pts\"]\ntags: []\n---\n",
    ]

    /// Build the built-in registry (mock backend). Parses each canonical page
    /// through the general frontmatter parser, identically to the synced path.
    static func buildBuiltins() -> PropertyRegistry {
        let notes = builtinPages.map { content -> RegistryNote in
            let custom = FrontmatterParser.parse(content: content)
            return RegistryNote(
                title: (custom["title"] as? String) ?? "",
                noteType: custom["type"] as? String,
                custom: custom
            )
        }
        return PropertyRegistry.build(from: notes)
    }
}
