import Foundation

/// Sort direction for a table column — mirrors web `table-sort.ts`'s
/// `SortDirection`.
enum TableSortDirection: String, Equatable {
    case asc
    case desc
}

/// Pure table logic for the iOS saved-views compact columnar table
/// (tesela-ya4.6, `.docs/ai/phases/2026-07-02-typesystem-views-spec.md`
/// decision 5 + gap G6). Mirrors the SETTLED web semantics
/// (`web/src/lib/table/table-columns.ts`, `table-config.ts`, `table-sort.ts`)
/// kept off the SwiftUI view — same "pure logic module + view" split as
/// `KanbanLogic`/`GrKanbanBoard` (tesela-ya4.5).
enum TableLogic {

    // MARK: - Column resolution (mirror web table-columns.ts::resolveTableColumns)

    /// Column candidates for the table: a tag-scoped table (the DSL has a
    /// positive `tag:X` filter, `KanbanLogic.inferredTag`) uses the TYPE's
    /// own declared property order (the FULL resolved defs — unlike
    /// Kanban's group-by candidates, a table column set isn't restricted to
    /// select-with-choices); a non-tag-scoped query has no single type to
    /// enumerate, so candidates are the global Property-page defs that
    /// actually appear on ≥1 returned item.
    static func resolveColumns(
        tagName: String?,
        items: [QueryItem],
        registry: PropertyRegistry
    ) -> [PropertyDef] {
        if let tagName {
            return registry.resolvedDefs(forTag: tagName)
        }
        var present = Set<String>()
        for item in items {
            for key in item.properties.keys { present.insert(key.lowercased()) }
        }
        return registry.properties.values
            .filter { present.contains($0.name.lowercased()) }
            .sorted { $0.name < $1.name }
    }

    // MARK: - Config application (mirror web table-config.ts::applyTableConfig)

    /// Project resolved columns through a saved view's stored display
    /// config: drop hidden columns, then apply the explicit `order`
    /// override. Columns not named in `order` (new columns since the
    /// config was last saved, or an empty `order`) append after the
    /// ordered ones, in their originally-resolved order — a stale/partial
    /// config never HIDES a column it doesn't mention, only reorders what
    /// it names. `nil` config is the "no override" default (mirrors web's
    /// `EMPTY_TABLE_CONFIG`) — returns `columns` unchanged.
    static func applyConfig(_ columns: [PropertyDef], config: SavedViewTableConfig?) -> [PropertyDef] {
        guard let config else { return columns }
        let visible = columns.filter { !config.hidden.contains($0.name) }
        guard !config.order.isEmpty else { return visible }
        var byName: [String: PropertyDef] = [:]
        for c in visible { byName[c.name] = c }
        var ordered: [PropertyDef] = []
        for name in config.order {
            if let c = byName[name] {
                ordered.append(c)
                byName.removeValue(forKey: name)
            }
        }
        for c in visible where byName[c.name] != nil {
            ordered.append(c)
        }
        return ordered
    }

    // MARK: - Cell value extraction

    /// Raw property value for a column on an item: exact key first, then
    /// lowercased fallback — mirror of Kanban's `column(for:...)` lookup
    /// and the web table's `getPropertyValue`. Empty string when absent.
    static func rawValue(for item: QueryItem, column: PropertyDef) -> String {
        item.properties[column.name] ?? item.properties[column.name.lowercased()] ?? ""
    }

    /// The formatted display text for a cell — routes through the shared
    /// `ChipFormat.formattedValue` (the SAME per-`value_type` formatting
    /// `PropertyChip`/web `DisplayChip` use for kanban-card/block-row
    /// property chips), so a property reads identically wherever it
    /// surfaces on iOS.
    static func cellText(for item: QueryItem, column: PropertyDef) -> String {
        ChipFormat.formattedValue(rawValue(for: item, column: column), def: column)
    }

    // MARK: - Sort (mirror web table-sort.ts)

    /// Typed comparison for one column's RAW values — mirror of web
    /// `compareTableValues`. Deliberately distinct from
    /// `LocalQueryEngine.compareValuesTyped` (the `ORDER BY` DSL-sort
    /// comparator used by saved-view queries): this ranks select /
    /// multi-select by DECLARED CHOICE ORDER (matching how the values read
    /// as `DisplayChip`'s bar format), which the DSL sort has no notion of.
    /// Returns a sign: negative when `a` sorts before `b`, positive after,
    /// zero for a tie.
    static func compare(_ a: String, _ b: String, valueType: PropertyType, choices: [String]) -> Int {
        switch valueType {
        case .number:
            let at = a.trimmingCharacters(in: .whitespaces)
            let bt = b.trimmingCharacters(in: .whitespaces)
            let an = Double(at)
            let bn = Double(bt)
            let aValid = !at.isEmpty && an != nil
            let bValid = !bt.isEmpty && bn != nil
            if aValid, bValid {
                if an! < bn! { return -1 }
                if an! > bn! { return 1 }
                return 0
            }
            if aValid { return -1 } // a valid number sorts before an empty/non-numeric value
            if bValid { return 1 }
            return localeCompare(a, b)
        case .checkbox:
            let ab = a.trimmingCharacters(in: .whitespaces).lowercased() == "true"
            let bb = b.trimmingCharacters(in: .whitespaces).lowercased() == "true"
            if ab == bb { return 0 }
            return ab ? 1 : -1 // unchecked/empty before checked
        case .select, .multiSelect:
            if !choices.isEmpty {
                func rank(_ v: String) -> Int {
                    let target = v.trimmingCharacters(in: .whitespaces).lowercased()
                    return choices.firstIndex { $0.lowercased() == target } ?? choices.count
                }
                let ar = rank(a)
                let br = rank(b)
                if ar != br { return ar - br }
            }
            return localeCompare(a, b)
        default:
            return localeCompare(a, b)
        }
    }

    /// Locale-aware string comparison mirroring JS `String.localeCompare`'s
    /// sign contract (-1/0/1).
    private static func localeCompare(_ a: String, _ b: String) -> Int {
        switch a.compare(b, options: [], range: nil, locale: Locale(identifier: "en_US")) {
        case .orderedAscending: return -1
        case .orderedDescending: return 1
        case .orderedSame: return 0
        }
    }

    /// Sort items by one resolved column, returning a NEW array (the input
    /// is never mutated) — mirror of web `sortByColumn`. `direction ==
    /// .desc` reverses the ascending result rather than negating the
    /// comparator, so the empty/invalid-value placement `compare` pins
    /// (e.g. an empty number sorts last) moves predictably to the other
    /// end instead of needing its own rule per direction.
    static func sortRows(_ items: [QueryItem], column: PropertyDef, direction: TableSortDirection) -> [QueryItem] {
        let sorted = items.sorted {
            compare(rawValue(for: $0, column: column), rawValue(for: $1, column: column), valueType: column.valueType, choices: column.choices) < 0
        }
        return direction == .asc ? sorted : Array(sorted.reversed())
    }
}
