import XCTest
@testable import Tesela

/// iOS native kanban view for saved views (tesela-ya4.5,
/// `.docs/ai/phases/2026-07-02-typesystem-views-spec.md` decision 3/5).
/// Exercises the pure logic behind `GrKanbanBoard`: group-by resolution
/// order, candidate-property derivation (tag-scoped vs. data-derived),
/// column/grouping derivation, and the move-target write value — kept off
/// the SwiftUI view per `KanbanLogic`'s own doc comment so these assertions
/// don't need to mount a view.
final class KanbanLogicTests: XCTestCase {

    // MARK: - Fixture builders

    private func note(_ title: String, _ noteType: String, _ custom: [String: Any]) -> RegistryNote {
        RegistryNote(title: title, noteType: noteType, custom: custom)
    }

    private func makeDef(
        name: String,
        valueType: PropertyType,
        choices: [String] = [],
        choiceColors: [String: String] = [:]
    ) -> PropertyDef {
        PropertyDef(
            name: name, valueType: valueType, choices: choices, def: nil, show: nil,
            hideByDefault: false, hideEmpty: true,
            chipIcon: nil, chipLabelMode: nil, chipShortLabel: nil, chipValueFormat: nil,
            chordKey: nil, valueChordKeys: [:], choiceColors: choiceColors, nlTriggers: []
        )
    }

    private func item(
        blockId: String,
        pageId: String = "daily-1",
        title: String = "Daily",
        text: String = "Some task",
        tag: String? = "Task",
        properties: [String: String] = [:]
    ) -> QueryItem {
        QueryItem(
            block_id: blockId, page_id: pageId, title: title, text: text,
            parent_breadcrumb: [], kind: .block, primary_tag: tag,
            properties: properties, page_note_type: "Daily"
        )
    }

    /// A Task tag declaring Status (select, 4 choices) + Priority (select,
    /// no choices — must be excluded as a candidate) + Notes (text — must
    /// be excluded).
    private var statusPage: RegistryNote {
        note("Status", "Property", [
            "value_type": "select",
            "choices": ["todo", "doing", "done", "blocked"],
        ])
    }
    private var priorityPage: RegistryNote {
        note("Priority", "Property", ["value_type": "select", "choices": [String]()])
    }
    private var notesPage: RegistryNote {
        note("Notes", "Property", ["value_type": "text"])
    }
    private var taskTag: RegistryNote {
        note("Task", "Tag", ["tag_properties": ["Status", "Priority", "Notes"]])
    }

    private func registry() -> PropertyRegistry {
        PropertyRegistry.build(from: [statusPage, priorityPage, notesPage, taskTag])
    }

    // MARK: - isSelectWithChoices

    func testIsSelectWithChoicesRequiresBothTypeAndNonEmptyChoices() {
        XCTAssertTrue(KanbanLogic.isSelectWithChoices(makeDef(name: "Status", valueType: .select, choices: ["a"])))
        XCTAssertFalse(KanbanLogic.isSelectWithChoices(makeDef(name: "Priority", valueType: .select, choices: [])))
        XCTAssertFalse(KanbanLogic.isSelectWithChoices(makeDef(name: "Notes", valueType: .text, choices: ["a"])))
    }

    // MARK: - resolveGroupBy (decision 3, iOS subset: a → c → nil)

    func testResolveGroupByPrefersExplicitDisplayGroupBy() {
        let candidates = [makeDef(name: "Priority", valueType: .select, choices: ["p1", "p2"])]
        let resolved = KanbanLogic.resolveGroupBy(
            displayGroupBy: "Status",
            candidates: candidates,
            resolveDef: { $0 == "Status" ? self.makeDef(name: "Status", valueType: .select, choices: ["todo", "done"]) : nil }
        )
        XCTAssertEqual(resolved, "Status")
    }

    func testResolveGroupByFallsBackToFirstCandidateWhenDisplayGroupByInvalid() {
        let candidates = [makeDef(name: "Priority", valueType: .select, choices: ["p1", "p2"])]
        let resolved = KanbanLogic.resolveGroupBy(
            displayGroupBy: "Deleted",
            candidates: candidates,
            resolveDef: { _ in nil }
        )
        XCTAssertEqual(resolved, "Priority")
    }

    func testResolveGroupByFallsBackWhenDisplayGroupByNilOrEmpty() {
        let candidates = [makeDef(name: "Priority", valueType: .select, choices: ["p1", "p2"])]
        XCTAssertEqual(
            KanbanLogic.resolveGroupBy(displayGroupBy: nil, candidates: candidates, resolveDef: { _ in nil }),
            "Priority"
        )
        XCTAssertEqual(
            KanbanLogic.resolveGroupBy(displayGroupBy: "", candidates: candidates, resolveDef: { _ in nil }),
            "Priority"
        )
    }

    func testResolveGroupByHonestNilWhenNothingResolves() {
        // Decision 3(d) — never a silent fallback; nil when no candidates
        // exist and no explicit override resolves.
        XCTAssertNil(KanbanLogic.resolveGroupBy(displayGroupBy: nil, candidates: [], resolveDef: { _ in nil }))
        XCTAssertNil(KanbanLogic.resolveGroupBy(displayGroupBy: "Ghost", candidates: [], resolveDef: { _ in nil }))
    }

    // MARK: - candidateProperties (decision 3c)

    func testTagScopedCandidatesUseTypeDeclaredOrderAndExcludeNonSelect() {
        let candidates = KanbanLogic.candidateProperties(tagName: "Task", items: [], registry: registry())
        // Priority (no choices) and Notes (text) are excluded; only Status
        // qualifies, in the type's declared `tag_properties` order.
        XCTAssertEqual(candidates.map(\.name), ["Status"])
    }

    func testNonTagScopedCandidatesUseGlobalPropertiesPresentOnReturnedItems() {
        let items = [
            item(blockId: "n1:0", properties: ["status": "todo"]),
            item(blockId: "n1:1", properties: ["notes": "free text"]),
        ]
        let candidates = KanbanLogic.candidateProperties(tagName: nil, items: items, registry: registry())
        // Status is select-with-choices AND present on a returned item.
        // Notes is present but not select-with-choices. Priority is
        // select-typed but has no choices AND isn't present on any item.
        XCTAssertEqual(candidates.map(\.name), ["Status"])
    }

    func testNonTagScopedCandidatesExcludeGlobalPropertiesAbsentFromResults() {
        // Status qualifies globally, but no returned item carries it — an
        // irrelevant global default is worse than the honest empty state
        // (decision 3d), so it must NOT be offered as a candidate.
        let items = [item(blockId: "n1:0", properties: ["notes": "free text"])]
        let candidates = KanbanLogic.candidateProperties(tagName: nil, items: items, registry: registry())
        XCTAssertTrue(candidates.isEmpty)
    }

    // MARK: - resolveDef

    func testResolveDefHonorsExplicitOverrideOutsideCandidateList() {
        // Priority isn't a candidate (no choices) — but resolveDef is used
        // to validate an explicit displayGroupBy/name independent of the
        // candidate list, so a property WITH choices declared elsewhere
        // must still resolve even if not currently offered.
        let reg = registry()
        XCTAssertNil(KanbanLogic.resolveDef("Priority", tagName: "Task", registry: reg), "no choices → not select-with-choices")
        XCTAssertNotNil(KanbanLogic.resolveDef("Status", tagName: "Task", registry: reg))
        XCTAssertNotNil(KanbanLogic.resolveDef("Status", tagName: nil, registry: reg), "falls back to the global registry")
        XCTAssertNil(KanbanLogic.resolveDef("Ghost", tagName: "Task", registry: reg))
    }

    // MARK: - columns

    func testColumnsPutsUnsetFirstThenChoiceOrder() {
        let def = makeDef(name: "Status", valueType: .select, choices: ["todo", "doing", "done"])
        XCTAssertEqual(KanbanLogic.columns(for: def), ["__unset__", "todo", "doing", "done"])
    }

    // MARK: - column(for:groupByProp:columns:)

    func testColumnLooksUpExactKeyThenLowercased() {
        let columns = ["__unset__", "todo", "doing", "done"]
        XCTAssertEqual(
            KanbanLogic.column(for: item(blockId: "n:0", properties: ["Status": "doing"]), groupByProp: "Status", columns: columns),
            "doing"
        )
        // Lowercased fallback — properties are often stored lowercased.
        XCTAssertEqual(
            KanbanLogic.column(for: item(blockId: "n:0", properties: ["status": "done"]), groupByProp: "Status", columns: columns),
            "done"
        )
    }

    func testColumnMissingEmptyOrUnknownValueLandsInUnset() {
        let columns = ["__unset__", "todo", "doing", "done"]
        XCTAssertEqual(
            KanbanLogic.column(for: item(blockId: "n:0", properties: [:]), groupByProp: "status", columns: columns),
            "__unset__"
        )
        XCTAssertEqual(
            KanbanLogic.column(for: item(blockId: "n:0", properties: ["status": ""]), groupByProp: "status", columns: columns),
            "__unset__"
        )
        XCTAssertEqual(
            KanbanLogic.column(for: item(blockId: "n:0", properties: ["status": "cancelled"]), groupByProp: "status", columns: columns),
            "__unset__",
            "an unrecognized value falls back to Unset, not a silently-invented column"
        )
    }

    // MARK: - grouped

    func testGroupedPreservesColumnOrderAndBucketsEachItemOnce() {
        let columns = ["__unset__", "todo", "doing", "done"]
        let items = [
            item(blockId: "n:0", properties: ["status": "doing"]),
            item(blockId: "n:1", properties: ["status": "todo"]),
            item(blockId: "n:2", properties: [:]),
            item(blockId: "n:3", properties: ["status": "todo"]),
        ]
        let grouped = KanbanLogic.grouped(items, groupByProp: "status", columns: columns)
        XCTAssertEqual(grouped.map(\.column), columns)
        XCTAssertEqual(grouped.first { $0.column == "todo" }?.items.map(\.block_id), ["n:1", "n:3"])
        XCTAssertEqual(grouped.first { $0.column == "doing" }?.items.map(\.block_id), ["n:0"])
        XCTAssertEqual(grouped.first { $0.column == "__unset__" }?.items.map(\.block_id), ["n:2"])
        XCTAssertEqual(grouped.first { $0.column == "done" }?.items.isEmpty, true)
    }

    // MARK: - writeValue(forColumn:)

    func testWriteValueClearsToEmptyStringForUnsetElseWritesTheColumn() {
        XCTAssertEqual(KanbanLogic.writeValue(forColumn: "__unset__"), "")
        XCTAssertEqual(KanbanLogic.writeValue(forColumn: "doing"), "doing")
    }

    // MARK: - inferredTag(fromDsl:) — mirror of web inferredKanbanTag

    func testInferredTagFindsPositiveTopLevelFilter() {
        XCTAssertEqual(KanbanLogic.inferredTag(fromDsl: "tag:Task"), "Task")
        XCTAssertEqual(KanbanLogic.inferredTag(fromDsl: "tag:Task status:todo"), "Task")
        // A `kind:` selector is consumed for its side-effect and doesn't
        // block finding the tag filter alongside it.
        XCTAssertEqual(KanbanLogic.inferredTag(fromDsl: "kind:block tag:Task"), "Task")
    }

    func testInferredTagNilWhenNoTagFilter() {
        XCTAssertNil(KanbanLogic.inferredTag(fromDsl: "status:todo"))
        XCTAssertNil(KanbanLogic.inferredTag(fromDsl: ""))
    }

    func testInferredTagNilForNegatedTagFilter() {
        // A negated tag filter isn't a POSITIVE scoping filter.
        XCTAssertNil(KanbanLogic.inferredTag(fromDsl: "-tag:Task"))
    }

    func testInferredTagNilInsideOrExpression() {
        // Mirror of web `flattenToLegacyFilters`: an OR at the top level
        // yields no filters at all — never guess which branch "counts".
        XCTAssertNil(KanbanLogic.inferredTag(fromDsl: "tag:Task OR tag:Project"))
    }

    func testInferredTagIgnoresMultiValueTagList() {
        // `tag:a,b` parses to an `inList` predicate, not a `cmp` — mirror
        // of web only matching `pred.kind === "cmp"`.
        XCTAssertNil(KanbanLogic.inferredTag(fromDsl: "tag:Task,Project"))
    }
}
