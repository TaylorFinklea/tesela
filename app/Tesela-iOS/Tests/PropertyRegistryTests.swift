import XCTest
@testable import Tesela

/// Phase 5.2 — iOS port of the web property-registry resolution tests
/// (`web/tests/unit/property-overrides.test.mjs`). The merged result MUST
/// equal what the web `getTagPropertyDefs`/`applyOverride` returns (which in
/// turn mirror the Rust `apply_override`), so these reuse the SAME shared
/// vectors the web/Rust tests assert on:
///   Task    Status == [todo, doing, done, blocked] + show on_new + default todo
///   Project Status == [planned, active, shipped]
///
/// Read layer only — no UI is exercised. Fixtures are built from `RegistryNote`
/// (title / note_type / custom), mirroring the web `note(title, type, custom)`
/// builder.
final class PropertyRegistryTests: XCTestCase {

    // MARK: - Fixture builders

    private func note(_ title: String, _ noteType: String, _ custom: [String: Any]) -> RegistryNote {
        RegistryNote(title: title, noteType: noteType, custom: custom)
    }

    /// The global Status Property page — choices are the GLOBAL fallback each
    /// type's `property_overrides.Status.choices` REPLACEs.
    private var statusPage: RegistryNote {
        note("Status", "Property", [
            "value_type": "select",
            "choices": ["backlog", "todo", "doing", "done"],
        ])
    }
    private var priorityPage: RegistryNote {
        note("Priority", "Property", [
            "value_type": "select",
            "choices": ["p1", "p2", "p3"],
            "nl_triggers": ["p1", "p2", "p3", "p4"],
        ])
    }
    private var deadlinePage: RegistryNote {
        note("Deadline", "Property", [
            "value_type": "date",
            "nl_triggers": ["due", "deadline"],
        ])
    }

    private var rootTag: RegistryNote { note("Root Tag", "Tag", ["tag_properties": [String]()]) }
    private var taskTag: RegistryNote {
        note("Task", "Tag", [
            "extends": "Root Tag",
            "tag_properties": ["Status", "Priority", "Deadline"],
            "property_overrides": [
                "Status": ["choices": ["todo", "doing", "done", "blocked"], "show": "on_new", "default": "todo"],
                "Priority": ["show": "on_set"],
            ],
        ])
    }
    private var projectTag: RegistryNote {
        note("Project", "Tag", [
            "extends": "Root Tag",
            "tag_properties": ["Status"],
            "property_overrides": ["Status": ["choices": ["planned", "active", "shipped"]]],
        ])
    }

    private func resolve(_ tag: String, _ notes: [RegistryNote]) -> [PropertyDef] {
        PropertyRegistry.build(from: notes).resolvedDefs(forTag: tag)
    }

    private func byName(_ defs: [PropertyDef], _ name: String) -> PropertyDef? {
        defs.first { $0.name.lowercased() == name.lowercased() }
    }

    // MARK: - Spec-required resolution cases

    func testTaskStatusReplacesGlobalShowOnNewDefaultTodo() {
        let defs = resolve("Task", [statusPage, priorityPage, deadlinePage, rootTag, taskTag, projectTag])
        let status = byName(defs, "Status")
        XCTAssertNotNil(status)
        XCTAssertEqual(status?.choices, ["todo", "doing", "done", "blocked"])
        XCTAssertEqual(status?.show, .onNew)
        XCTAssertEqual(status?.def, "todo")
    }

    func testProjectStatusReplacesShowDerivedOnNew() {
        let defs = resolve("Project", [statusPage, priorityPage, rootTag, taskTag, projectTag])
        let status = byName(defs, "Status")
        XCTAssertEqual(status?.choices, ["planned", "active", "shipped"])
        // No show override + Status.hide_by_default=false → derived on_new.
        XCTAssertEqual(status?.show, .onNew)
    }

    func testTaskPriorityShowOnSetKeepsGlobalChoicesAndNlTriggers() {
        let defs = resolve("Task", [statusPage, priorityPage, deadlinePage, rootTag, taskTag, projectTag])
        let priority = byName(defs, "Priority")
        XCTAssertNotNil(priority)
        XCTAssertEqual(priority?.show, .onSet)
        XCTAssertEqual(priority?.choices, ["p1", "p2", "p3"])
        XCTAssertEqual(priority?.valueType, .select)
        XCTAssertEqual(priority?.nlTriggers, ["p1", "p2", "p3", "p4"])
    }

    func testDeadlineNlTriggers() {
        let defs = resolve("Task", [statusPage, priorityPage, deadlinePage, rootTag, taskTag, projectTag])
        let deadline = byName(defs, "Deadline")
        XCTAssertNotNil(deadline)
        XCTAssertEqual(deadline?.valueType, .date)
        XCTAssertEqual(deadline?.nlTriggers, ["due", "deadline"])
    }

    func testNoOverrideTagChoicesIdenticalToGlobalShowDerivedOnNew() {
        let personTag = note("Person", "Tag", [
            "extends": "Root Tag",
            "tag_properties": ["Status"],
        ])
        let defs = resolve("Person", [statusPage, rootTag, personTag])
        let status = byName(defs, "Status")
        XCTAssertEqual(status?.choices, ["backlog", "todo", "doing", "done"])
        XCTAssertEqual(status?.show, .onNew)
        XCTAssertNil(status?.def)
    }

    func testHideByDefaultDerivesShowHidden() {
        let hiddenProp = note("Secret", "Property", ["value_type": "text", "hide_by_default": true])
        let tag = note("Vault", "Tag", ["extends": "Root Tag", "tag_properties": ["Secret"]])
        let defs = resolve("Vault", [hiddenProp, rootTag, tag])
        XCTAssertEqual(byName(defs, "Secret")?.show, .hidden)
    }

    // MARK: - REPLACE / SUBTRACT

    func testReplaceThenSubtract() {
        let tag = note("Triage", "Tag", [
            "extends": "Root Tag",
            "tag_properties": ["Status"],
            "property_overrides": [
                "Status": ["choices": ["todo", "doing", "done", "blocked"], "hide_choices": ["blocked"]],
            ],
        ])
        let defs = resolve("Triage", [statusPage, rootTag, tag])
        // REPLACE [todo,doing,done,blocked] then SUBTRACT [blocked].
        XCTAssertEqual(byName(defs, "Status")?.choices, ["todo", "doing", "done"])
    }

    func testLegacyHiddenPropSubtractsAfterReplace() {
        let tag = note("Triage2", "Tag", [
            "extends": "Root Tag",
            "tag_properties": ["Status"],
            "property_overrides": ["Status": ["choices": ["todo", "doing", "done", "blocked"]]],
            "hidden_Status": ["done"],
        ])
        let defs = resolve("Triage2", [statusPage, rootTag, tag])
        XCTAssertEqual(byName(defs, "Status")?.choices, ["todo", "doing", "blocked"])
    }

    func testChildWinsFirstInsert() {
        let parent = note("Base", "Tag", [
            "tag_properties": ["Status"],
            "property_overrides": ["Status": ["choices": ["a", "b"], "default": "a", "show": "hidden"]],
        ])
        let child = note("Derived", "Tag", [
            "extends": "Base",
            "tag_properties": [String](),
            "property_overrides": ["Status": ["choices": ["x", "y"], "default": "x", "show": "on_set"]],
        ])
        let defs = resolve("Derived", [statusPage, parent, child])
        let status = byName(defs, "Status")
        XCTAssertEqual(status?.choices, ["x", "y"])
        XCTAssertEqual(status?.def, "x")
        XCTAssertEqual(status?.show, .onSet)
    }

    func testLegacyHiddenAdditiveAcrossChain() {
        let parent = note("PBase", "Tag", ["tag_properties": ["Status"], "hidden_Status": ["d"]])
        let child = note("PDerived", "Tag", [
            "extends": "PBase",
            "tag_properties": ["Status"],
            "property_overrides": ["Status": ["choices": ["a", "b", "c", "d"]]],
            "hidden_Status": ["a"],
        ])
        let defs = resolve("PDerived", [statusPage, parent, child])
        // REPLACE [a,b,c,d] then SUBTRACT {a (child), d (parent)} → [b, c].
        XCTAssertEqual(byName(defs, "Status")?.choices, ["b", "c"])
    }

    func testWholeOverrideFirstInsertWinsChoicesNullDiscardsParent() {
        let parent = note("WBase", "Tag", [
            "tag_properties": ["Status"],
            "property_overrides": ["Status": ["choices": ["a", "b", "c", "d"]]],
        ])
        let child = note("WDerived", "Tag", [
            "extends": "WBase",
            "tag_properties": [String](),
            "property_overrides": ["Status": ["hide_choices": ["backlog"]]],
        ])
        let defs = resolve("WDerived", [statusPage, parent, child])
        // child override wins entirely (choices null → keep GLOBAL), then
        // subtract its own hide_choices ["backlog"].
        XCTAssertEqual(byName(defs, "Status")?.choices, ["todo", "doing", "done"])
    }

    func testOverrideForPropertyNotInMembershipIgnored() {
        let tag = note("Lonely", "Tag", [
            "extends": "Root Tag",
            "tag_properties": ["Status"],
            "property_overrides": [
                "Status": ["choices": ["todo"]],
                "Priority": ["choices": ["zzz"]],
            ],
        ])
        let defs = resolve("Lonely", [statusPage, priorityPage, rootTag, tag])
        XCTAssertNil(byName(defs, "Priority"))
        XCTAssertEqual(byName(defs, "Status")?.choices, ["todo"])
    }

    func testOverrideForPropertyWithNoGlobalPageAppliesToTextStub() {
        let tag = note("Stubby", "Tag", [
            "extends": "Root Tag",
            "tag_properties": ["Phase"], // no Phase Property page exists
            "property_overrides": ["Phase": ["choices": ["alpha", "beta"], "default": "alpha", "show": "on_set"]],
        ])
        let defs = resolve("Stubby", [rootTag, tag])
        let phase = byName(defs, "Phase")
        XCTAssertNotNil(phase)
        XCTAssertEqual(phase?.valueType, .text)
        XCTAssertEqual(phase?.choices, ["alpha", "beta"])
        XCTAssertEqual(phase?.def, "alpha")
        XCTAssertEqual(phase?.show, .onSet)
    }

    func testCaseInsensitiveOverrideKeys() {
        let tag = note("CaseTag", "Tag", [
            "extends": "Root Tag",
            "tag_properties": ["Status"],
            "property_overrides": ["STATUS": ["choices": ["x"]]],
        ])
        let defs = resolve("CaseTag", [statusPage, rootTag, tag])
        XCTAssertEqual(byName(defs, "Status")?.choices, ["x"])
    }

    func testMalformedNonObjectOverrideIgnored() {
        let tag = note("Bad", "Tag", [
            "extends": "Root Tag",
            "tag_properties": ["Status"],
            "property_overrides": ["Status": "garbage"],
        ])
        let defs = resolve("Bad", [statusPage, rootTag, tag])
        XCTAssertEqual(byName(defs, "Status")?.choices, ["backlog", "todo", "doing", "done"])
        XCTAssertEqual(byName(defs, "Status")?.show, .onNew)
    }

    func testCycleSafeInheritanceChain() {
        // a → b → a cycle: chain stops, no infinite loop.
        let a = note("A", "Tag", ["extends": "B", "tag_properties": ["Status"]])
        let b = note("B", "Tag", ["extends": "A", "tag_properties": [String]()])
        let reg = PropertyRegistry.build(from: [statusPage, a, b])
        let chain = reg.tagChain("A")
        XCTAssertEqual(chain, ["a", "b"])
        // Still resolves Status without hanging.
        XCTAssertEqual(byName(reg.resolvedDefs(forTag: "A"), "Status")?.choices,
                       ["backlog", "todo", "doing", "done"])
    }

    // MARK: - resolvedType header (icon / plural)

    func testResolvedTypeIconAndPlural() {
        let tag = note("Task", "Tag", [
            "extends": "Root Tag",
            "icon": "checkbox",
            "plural": "Tasks",
            "tag_properties": ["Status"],
        ])
        let reg = PropertyRegistry.build(from: [statusPage, rootTag, tag])
        let type = reg.resolvedType(forTag: "Task")
        XCTAssertEqual(type.name, "Task")
        XCTAssertEqual(type.plural, "Tasks")
        XCTAssertEqual(type.icon, "checkbox")
        XCTAssertNotNil(type.properties.first { $0.name == "Status" })
    }

    func testResolvedTypePluralFallsBackToName() {
        let tag = note("Widget", "Tag", ["extends": "Root Tag", "tag_properties": [String]()])
        let reg = PropertyRegistry.build(from: [rootTag, tag])
        XCTAssertEqual(reg.resolvedType(forTag: "Widget").plural, "Widget")
    }

    // MARK: - Frontmatter parser (nested YAML the single-line scrapers can't)

    func testFrontmatterParsesFlowArraysAndMaps() {
        let content = """
        ---
        title: "Task"
        type: "Tag"
        extends: "Root Tag"
        tag_properties: ["Status", "Priority"]
        property_overrides: {Status: {choices: [todo, doing, done, blocked], show: on_new, default: todo}}
        value_chord_keys: {b: backlog, t: todo}
        nl_triggers: [due, deadline]
        hide_by_default: true
        ---
        - body
        """
        let c = FrontmatterParser.parse(content: content)
        XCTAssertEqual(c["type"] as? String, "Tag")
        XCTAssertEqual((c["tag_properties"] as? [Any])?.compactMap { $0 as? String }, ["Status", "Priority"])
        XCTAssertEqual((c["nl_triggers"] as? [Any])?.compactMap { $0 as? String }, ["due", "deadline"])
        XCTAssertEqual(c["hide_by_default"] as? Bool, true)
        let over = c["property_overrides"] as? [String: Any]
        let status = over?["Status"] as? [String: Any]
        XCTAssertEqual((status?["choices"] as? [Any])?.compactMap { $0 as? String }, ["todo", "doing", "done", "blocked"])
        XCTAssertEqual(status?["show"] as? String, "on_new")
        XCTAssertEqual(status?["default"] as? String, "todo")
        let vck = c["value_chord_keys"] as? [String: Any]
        XCTAssertEqual(vck?["b"] as? String, "backlog")
    }

    func testFrontmatterParsesCompactJSONFlowMap() {
        // The web writes property_overrides as inline JSON — must parse as flow YAML.
        let content = """
        ---
        title: "Triage"
        type: "Tag"
        tag_properties: ["Status"]
        property_overrides: {"Status": {"choices": ["a", "b"], "show": "hidden"}}
        ---
        """
        let c = FrontmatterParser.parse(content: content)
        let over = c["property_overrides"] as? [String: Any]
        let status = over?["Status"] as? [String: Any]
        XCTAssertEqual((status?["choices"] as? [Any])?.compactMap { $0 as? String }, ["a", "b"])
        XCTAssertEqual(status?["show"] as? String, "hidden")
    }

    /// End-to-end through the parser: the canonical built-in Task/Status pages
    /// (parsed from real frontmatter strings) resolve to the spec vector.
    func testBuiltinsResolveTaskStatus() {
        let reg = PropertyRegistry.buildBuiltins()
        let defs = reg.resolvedDefs(forTag: "Task")
        let status = byName(defs, "Status")
        XCTAssertEqual(status?.choices, ["todo", "doing", "done", "blocked"])
        XCTAssertEqual(status?.show, .onNew)
        XCTAssertEqual(status?.def, "todo")
        let priority = byName(defs, "Priority")
        XCTAssertEqual(priority?.nlTriggers, ["p1", "p2", "p3", "p4"])
        let deadline = byName(defs, "Deadline")
        XCTAssertEqual(deadline?.nlTriggers, ["due", "deadline"])
    }

    func testBuiltinsResolveProjectStatus() {
        let reg = PropertyRegistry.buildBuiltins()
        let status = byName(reg.resolvedDefs(forTag: "Project"), "Status")
        XCTAssertEqual(status?.choices, ["planned", "active", "shipped"])
        XCTAssertEqual(status?.show, .onNew)
    }

    // MARK: - typeNames (capture type picker)

    /// `typeNames()` lists every Tag page except the abstract "Root Tag"
    /// base, sorted. Drives the Capture composer's type picker.
    func testTypeNamesListsTagPagesExcludingRootTag() {
        let reg = PropertyRegistry.build(from: [statusPage, priorityPage, rootTag, taskTag, projectTag])
        XCTAssertEqual(reg.typeNames(), ["Project", "Task"])
    }

    /// Property pages are NOT types — only Tag pages are offerable.
    func testTypeNamesExcludesPropertyPages() {
        let reg = PropertyRegistry.build(from: [statusPage, priorityPage, deadlinePage, rootTag, taskTag])
        XCTAssertEqual(reg.typeNames(), ["Task"])
    }

    /// The built-in registry always offers Task + Project, so a
    /// not-yet-synced registry still gives the picker something to show.
    func testBuiltinTypeNamesIncludeTaskAndProject() {
        let names = PropertyRegistry.buildBuiltins().typeNames()
        XCTAssertTrue(names.contains("Task"))
        XCTAssertTrue(names.contains("Project"))
        XCTAssertFalse(names.contains("Root Tag"))
    }

    /// An empty registry yields no types (the caller falls back to builtins).
    func testTypeNamesEmptyOnEmptyRegistry() {
        XCTAssertTrue(PropertyRegistry().typeNames().isEmpty)
    }
}
