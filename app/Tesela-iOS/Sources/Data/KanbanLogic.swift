import Foundation

/// Pure kanban logic for the iOS saved-views kanban surface (tesela-ya4.5,
/// `.docs/ai/phases/2026-07-02-typesystem-views-spec.md` decision 5). Mirrors
/// the SETTLED web semantics (`web/src/lib/kanban-group-by.ts` +
/// `KanbanBoard.svelte`'s column/grouping logic), kept off the SwiftUI view
/// so the acceptance-critical semantics — group-by resolution order, column
/// derivation, move-target write value — are unit-testable without mounting
/// a view.
///
/// `GrInboxView` is ALWAYS a saved-view context (there is no iOS tag-page
/// kanban), so decision 3's (b) "per-surface localStorage pref" never
/// applies here — mirrors how `KanbanBoard.svelte` itself gates (b) off
/// whenever `viewId` is set. Only (a), (c), (d) are implemented.
enum KanbanLogic {
    /// The sentinel "no value" column key — mirrors the web `__unset__`.
    static let unsetColumn = "__unset__"

    /// A property is a valid kanban group-by column source only if it's
    /// select-typed AND has at least one declared choice — mirror of web
    /// `isSelectWithChoices`.
    static func isSelectWithChoices(_ def: PropertyDef) -> Bool {
        def.valueType == .select && !def.choices.isEmpty
    }

    /// Group-by resolution (spec decision 3, iOS subset):
    ///   (a) explicit `display_group_by` on the active saved view, when it
    ///       resolves to a select-with-choices property
    ///   (c) the first select-with-choices candidate
    ///   (d) honest `nil` — never a silent list fallback
    static func resolveGroupBy(
        displayGroupBy: String?,
        candidates: [PropertyDef],
        resolveDef: (String) -> PropertyDef?
    ) -> String? {
        if let displayGroupBy, !displayGroupBy.isEmpty, resolveDef(displayGroupBy) != nil {
            return displayGroupBy
        }
        return candidates.first?.name
    }

    /// Group-by candidates for decision-3(c): a tag-scoped board (the DSL
    /// has a positive `tag:X` filter) uses the TYPE's own declared property
    /// order (mirror of tag-page kanban); a non-tag-scoped query has no
    /// single type to enumerate, so candidates are the global Property-page
    /// defs that actually appear on ≥ 1 returned item (mirror of
    /// `KanbanBoard.svelte`'s `selectProperties` for the non-tag-scoped
    /// branch). Sorted by name for a deterministic, testable order — the
    /// registry's own dictionary has no ordering guarantee.
    static func candidateProperties(
        tagName: String?,
        items: [QueryItem],
        registry: PropertyRegistry
    ) -> [PropertyDef] {
        if let tagName {
            return registry.resolvedDefs(forTag: tagName).filter(isSelectWithChoices)
        }
        var present = Set<String>()
        for item in items {
            for key in item.properties.keys { present.insert(key.lowercased()) }
        }
        return registry.properties.values
            .filter { isSelectWithChoices($0) && present.contains($0.name.lowercased()) }
            .sorted { $0.name < $1.name }
    }

    /// Group-by candidates for the saved-view EDITOR's picker (tesela-ya4.7,
    /// spec decision 4/G7) — `GrViewEditorSheet` has a draft DSL but never
    /// runs a query, so it has no `items` to pass to `candidateProperties`.
    /// For a tag-scoped draft this defers straight to `candidateProperties`
    /// (that branch never reads `items`, so it's identical to what the live
    /// board would show). For a non-tag-scoped draft, `candidateProperties`
    /// would always return `[]` (its `items`-presence filter can never pass
    /// with an empty items list) — so this falls back to every
    /// select-with-choices property in the registry instead. That fallback
    /// is a strict SUPERSET of what the board would offer, never a mismatch
    /// that could make a picked value silently no-op: `resolveDef` (the
    /// actual accept gate the board applies to `displayGroupBy` at render
    /// time) doesn't filter by `candidateProperties` membership at all for
    /// a non-tag-scoped view — it only requires the picked name to resolve
    /// to a select-with-choices def somewhere in the registry.
    static func editorCandidateProperties(
        tagName: String?,
        registry: PropertyRegistry
    ) -> [PropertyDef] {
        if let tagName {
            return candidateProperties(tagName: tagName, items: [], registry: registry)
        }
        return registry.properties.values
            .filter(isSelectWithChoices)
            .sorted { $0.name < $1.name }
    }

    /// Resolve ANY property name (not just a `candidateProperties` member)
    /// to its select-with-choices def — an explicit `displayGroupBy` must be
    /// honored even when it isn't in the candidate list (decision 3a
    /// outranks "does the data have it"). `nil` when the name doesn't exist
    /// or isn't select-type-with-choices.
    static func resolveDef(
        _ name: String,
        tagName: String?,
        registry: PropertyRegistry
    ) -> PropertyDef? {
        let fromType = tagName.flatMap { tag in
            registry.resolvedDefs(forTag: tag).first { $0.name == name }
        }
        guard let def = fromType ?? registry.properties[name.lowercased()],
              isSelectWithChoices(def) else {
            return nil
        }
        return def
    }

    /// Column keys: `__unset__` first, then the resolved property's
    /// canonical choice order — mirror of `columnNames` in
    /// `KanbanBoard.svelte`.
    static func columns(for def: PropertyDef) -> [String] {
        [unsetColumn] + def.choices
    }

    /// The column a query item currently belongs in: case-sensitive
    /// property lookup first, then the lowercased key; an empty, missing,
    /// or unrecognized value all land in `__unset__` — mirror of
    /// `groupedBlocks` in `KanbanBoard.svelte` ("unknown value goes to
    /// unset").
    static func column(for item: QueryItem, groupByProp: String, columns: [String]) -> String {
        let val = item.properties[groupByProp] ?? item.properties[groupByProp.lowercased()] ?? ""
        if val.isEmpty { return unsetColumn }
        return columns.contains(val) ? val : unsetColumn
    }

    /// Group items into an ordered `(column, items)` list, ordered by
    /// `columns` so the board never has to re-sort at render time.
    static func grouped(
        _ items: [QueryItem],
        groupByProp: String,
        columns: [String]
    ) -> [(column: String, items: [QueryItem])] {
        var map: [String: [QueryItem]] = [:]
        for col in columns { map[col] = [] }
        for item in items {
            let col = column(for: item, groupByProp: groupByProp, columns: columns)
            map[col, default: []].append(item)
        }
        return columns.map { (column: $0, items: map[$0] ?? []) }
    }

    /// The value to WRITE via `setBlockProperty` when moving a card into
    /// `column`. `__unset__` writes an empty string rather than requiring a
    /// separate clear-property call — the grouping logic above already
    /// treats an empty property value as unset, so an empty-string write is
    /// sufficient to land the card back in the Unset column (mirror of the
    /// bead's "writing status/select via setBlockProperty").
    static func writeValue(forColumn column: String) -> String {
        column == unsetColumn ? "" : column
    }

    /// Extract the first POSITIVE top-level `tag:X` predicate from a DSL —
    /// mirror of web `inferredKanbanTag` (`QueryWidgetView.svelte`) /
    /// `flattenToLegacyFilters` (`query-language.ts`): only a flat
    /// AND-of-atoms (or a single bare atom) at the TOP level is examined: an
    /// `OR` expression, or an atom nested inside a parenthesized group,
    /// yields `nil` — the same "structural, not best-effort" posture the
    /// web flattener takes. Gives the board the type's own declared
    /// property order (decision 3c) and distinguishes tag-scoped from
    /// data-derived candidate resolution.
    static func inferredTag(fromDsl dsl: String) -> String? {
        firstPositiveTag(LocalQueryEngine.parseSimpleDsl(dsl).expr)
    }

    private static func firstPositiveTag(_ expr: LocalQueryEngine.SimpleDsl.BoolExpr) -> String? {
        let atoms: [LocalQueryEngine.SimpleDsl.BoolExpr]
        switch expr {
        case .and(let args): atoms = args
        case .or(_): return nil
        default: atoms = [expr]
        }
        for a in atoms {
            if case .atom(let pred) = a,
               case .cmp(let key, let op, let value) = pred,
               key == "tag", op == .eq {
                return value
            }
        }
        return nil
    }
}
