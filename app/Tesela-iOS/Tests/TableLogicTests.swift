import XCTest
@testable import Tesela

/// iOS native compact columnar table for saved views (tesela-ya4.6,
/// `.docs/ai/phases/2026-07-02-typesystem-views-spec.md` decision 5 + gap
/// G6). Exercises the pure logic behind `GrTableView`: column resolution
/// (tag-scoped vs. data-derived), `SavedViewTableConfig` application
/// (hide/reorder, incl. nil-config defaults), raw cell-value extraction,
/// and the typed sort comparator — kept off the SwiftUI view per
/// `TableLogic`'s own doc comment so these assertions don't need to mount
/// a view. Mirrors `KanbanLogicTests`' fixture style.
final class TableLogicTests: XCTestCase {

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

    /// A Task tag declaring Status (select), Priority (select), and Notes
    /// (text) — unlike Kanban's candidates, table columns are NOT filtered
    /// to select-with-choices, so all three should resolve for a tag-scoped
    /// table.
    private var statusPage: RegistryNote {
        note("Status", "Property", [
            "value_type": "select",
            "choices": ["todo", "doing", "done", "blocked"],
        ])
    }
    private var priorityPage: RegistryNote {
        note("Priority", "Property", ["value_type": "select", "choices": ["p1", "p2", "p3"]])
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

    // MARK: - resolveColumns (mirror web resolveTableColumns)

    func testTagScopedColumnsUseTypeDeclaredOrderIncludingNonSelect() {
        let columns = TableLogic.resolveColumns(tagName: "Task", items: [], registry: registry())
        // Unlike Kanban's candidates, table columns aren't restricted to
        // select-with-choices — Notes (text) is included, in declared order.
        XCTAssertEqual(columns.map(\.name), ["Status", "Priority", "Notes"])
    }

    func testNonTagScopedColumnsUseGlobalPropertiesPresentOnReturnedItems() {
        let items = [
            item(blockId: "n1:0", properties: ["status": "todo"]),
            item(blockId: "n1:1", properties: ["notes": "free text"]),
        ]
        let columns = TableLogic.resolveColumns(tagName: nil, items: items, registry: registry())
        // Priority is a known global property but absent from every
        // returned item — excluded (mirror kanban's decision-3c rationale:
        // an irrelevant global default is worse than a tight column set).
        XCTAssertEqual(columns.map(\.name), ["Notes", "Status"], "sorted by name for determinism")
    }

    func testNonTagScopedColumnsEmptyWhenNoPropertiesPresent() {
        let items = [item(blockId: "n1:0", properties: [:])]
        XCTAssertTrue(TableLogic.resolveColumns(tagName: nil, items: items, registry: registry()).isEmpty)
    }

    // MARK: - applyConfig (mirror web applyTableConfig)

    func testApplyConfigNilReturnsColumnsUnchanged() {
        let columns = [makeDef(name: "Status", valueType: .select), makeDef(name: "Priority", valueType: .select)]
        XCTAssertEqual(TableLogic.applyConfig(columns, config: nil).map(\.name), ["Status", "Priority"])
    }

    func testApplyConfigDropsHiddenColumns() {
        let columns = [makeDef(name: "Status", valueType: .select), makeDef(name: "Priority", valueType: .select), makeDef(name: "Notes", valueType: .text)]
        let config = SavedViewTableConfig(hidden: ["Priority"], order: [], sortBy: nil, sortDir: nil)
        XCTAssertEqual(TableLogic.applyConfig(columns, config: config).map(\.name), ["Status", "Notes"])
    }

    func testApplyConfigOrdersNamedColumnsFirstThenAppendsRest() {
        let columns = [makeDef(name: "Status", valueType: .select), makeDef(name: "Priority", valueType: .select), makeDef(name: "Notes", valueType: .text)]
        let config = SavedViewTableConfig(hidden: [], order: ["Notes", "Status"], sortBy: nil, sortDir: nil)
        // Priority isn't named in `order` — appends after the ordered ones,
        // in its originally-resolved position (mirror web: never HIDES a
        // column it doesn't mention).
        XCTAssertEqual(TableLogic.applyConfig(columns, config: config).map(\.name), ["Notes", "Status", "Priority"])
    }

    func testApplyConfigOrderIgnoresStaleNamesNotInVisibleColumns() {
        let columns = [makeDef(name: "Status", valueType: .select), makeDef(name: "Priority", valueType: .select)]
        let config = SavedViewTableConfig(hidden: [], order: ["Ghost", "Priority"], sortBy: nil, sortDir: nil)
        XCTAssertEqual(TableLogic.applyConfig(columns, config: config).map(\.name), ["Priority", "Status"])
    }

    func testApplyConfigEmptyOrderReturnsVisibleColumnsUnreordered() {
        let columns = [makeDef(name: "Status", valueType: .select), makeDef(name: "Priority", valueType: .select)]
        let config = SavedViewTableConfig(hidden: [], order: [], sortBy: nil, sortDir: nil)
        XCTAssertEqual(TableLogic.applyConfig(columns, config: config).map(\.name), ["Status", "Priority"])
    }

    // MARK: - rawValue (typed cell value extraction)

    func testRawValueLooksUpExactKeyThenLowercased() {
        let col = makeDef(name: "Status", valueType: .select, choices: ["todo", "done"])
        XCTAssertEqual(TableLogic.rawValue(for: item(blockId: "n:0", properties: ["Status": "done"]), column: col), "done")
        XCTAssertEqual(TableLogic.rawValue(for: item(blockId: "n:0", properties: ["status": "todo"]), column: col), "todo")
    }

    func testRawValueEmptyWhenPropertyAbsent() {
        let col = makeDef(name: "Status", valueType: .select)
        XCTAssertEqual(TableLogic.rawValue(for: item(blockId: "n:0", properties: [:]), column: col), "")
    }

    // MARK: - cellText (per-value_type formatting, delegates to ChipFormat)

    func testCellTextFormatsDateAsMonthDay() {
        let col = makeDef(name: "Deadline", valueType: .date)
        let text = TableLogic.cellText(for: item(blockId: "n:0", properties: ["Deadline": "2026-03-14"]), column: col)
        XCTAssertEqual(text, DateFormat.humanMonthDay("2026-03-14"))
    }

    func testCellTextEmptyWhenValueAbsent() {
        let col = makeDef(name: "Notes", valueType: .text)
        XCTAssertEqual(TableLogic.cellText(for: item(blockId: "n:0", properties: [:]), column: col), "")
    }

    // MARK: - compare (mirror web compareTableValues)

    func testCompareNumberRanksValidNumbersNumerically() {
        XCTAssertLessThan(TableLogic.compare("2", "10", valueType: .number, choices: []), 0)
        XCTAssertGreaterThan(TableLogic.compare("10", "2", valueType: .number, choices: []), 0)
        XCTAssertEqual(TableLogic.compare("5", "5", valueType: .number, choices: []), 0)
    }

    func testCompareNumberValidSortsBeforeInvalidOrEmpty() {
        XCTAssertLessThan(TableLogic.compare("5", "", valueType: .number, choices: []), 0)
        XCTAssertGreaterThan(TableLogic.compare("", "5", valueType: .number, choices: []), 0)
        XCTAssertLessThan(TableLogic.compare("5", "not-a-number", valueType: .number, choices: []), 0)
    }

    func testCompareCheckboxUncheckedBeforeChecked() {
        XCTAssertLessThan(TableLogic.compare("false", "true", valueType: .checkbox, choices: []), 0)
        XCTAssertGreaterThan(TableLogic.compare("true", "false", valueType: .checkbox, choices: []), 0)
        XCTAssertEqual(TableLogic.compare("true", "true", valueType: .checkbox, choices: []), 0)
    }

    func testCompareSelectRanksByDeclaredChoiceOrder() {
        let choices = ["todo", "doing", "done"]
        XCTAssertLessThan(TableLogic.compare("todo", "done", valueType: .select, choices: choices), 0)
        XCTAssertGreaterThan(TableLogic.compare("done", "doing", valueType: .select, choices: choices), 0)
    }

    func testCompareSelectOffListValueRanksLastByChoiceCount() {
        let choices = ["todo", "doing", "done"]
        XCTAssertLessThan(TableLogic.compare("done", "cancelled", valueType: .select, choices: choices), 0)
    }

    func testCompareDefaultFallsBackToLocaleCompare() {
        XCTAssertLessThan(TableLogic.compare("apple", "banana", valueType: .text, choices: []), 0)
        XCTAssertEqual(TableLogic.compare("same", "same", valueType: .text, choices: []), 0)
    }

    // MARK: - sortRows (mirror web sortByColumn)

    func testSortRowsAscendingByNumber() {
        let col = makeDef(name: "Points", valueType: .number)
        let items = [
            item(blockId: "a", properties: ["Points": "3"]),
            item(blockId: "b", properties: ["Points": "1"]),
            item(blockId: "c", properties: ["Points": "2"]),
        ]
        let sorted = TableLogic.sortRows(items, column: col, direction: .asc)
        XCTAssertEqual(sorted.map(\.block_id), ["b", "c", "a"])
    }

    func testSortRowsDescendingReversesAscendingResult() {
        let col = makeDef(name: "Points", valueType: .number)
        let items = [
            item(blockId: "a", properties: ["Points": "3"]),
            item(blockId: "b", properties: ["Points": "1"]),
            item(blockId: "c", properties: ["Points": ""]),
        ]
        // Ascending: valid numbers first (low→high), then the empty value
        // last — descending reverses that whole ordering, so the empty
        // value moves to the FRONT rather than being re-ranked.
        let sorted = TableLogic.sortRows(items, column: col, direction: .desc)
        XCTAssertEqual(sorted.map(\.block_id), ["c", "a", "b"])
    }

    func testSortRowsDoesNotMutateInput() {
        let col = makeDef(name: "Points", valueType: .number)
        let items = [
            item(blockId: "a", properties: ["Points": "3"]),
            item(blockId: "b", properties: ["Points": "1"]),
        ]
        _ = TableLogic.sortRows(items, column: col, direction: .asc)
        XCTAssertEqual(items.map(\.block_id), ["a", "b"], "the input array is never mutated")
    }
}
