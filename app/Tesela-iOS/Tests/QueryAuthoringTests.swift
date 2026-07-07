import XCTest
@testable import Tesela

/// Pure-logic tests for the iOS JQL saved-view authoring UX
/// (tesela-vp9.5): caret-context tier classification, completion
/// candidate assembly against a fake property registry, the chip
/// registry's colon-DSL-fragment ↔ JQL-clause equivalence, and the
/// parse-aware `SavedViewLogic.toggleFragment`/`fragmentActive` round
/// trip.
@MainActor
final class QueryAuthoringTests: XCTestCase {

    // MARK: - Caret-context classification

    func testCaretContextEmptyInputIsKeyTier() {
        let ctx = QueryAuthoring.caretContext("", cursor: 0)
        XCTAssertEqual(ctx.tier, .key)
        XCTAssertNil(ctx.key)
    }

    func testCaretContextMidKeyWordIsKeyTierWithPrefix() {
        // Still typing the key itself — offer key-name completions
        // filtered by what's typed so far.
        let ctx = QueryAuthoring.caretContext("stat", cursor: 4)
        XCTAssertEqual(ctx.tier, .key)
        XCTAssertEqual(ctx.prefix, "stat")
        XCTAssertEqual(ctx.from, 0)
    }

    func testCaretContextAfterKeyWithSpaceIsOperatorTier() {
        let ctx = QueryAuthoring.caretContext("status ", cursor: 7)
        XCTAssertEqual(ctx.tier, .operatorTier)
        XCTAssertEqual(ctx.key, "status")
    }

    func testCaretContextAfterColonIsValueTier() {
        let ctx = QueryAuthoring.caretContext("status:", cursor: 7)
        XCTAssertEqual(ctx.tier, .value)
        XCTAssertEqual(ctx.key, "status")
    }

    func testCaretContextMidValueWordIsValueTierWithPrefix() {
        let ctx = QueryAuthoring.caretContext("status:tod", cursor: 10)
        XCTAssertEqual(ctx.tier, .value)
        XCTAssertEqual(ctx.key, "status")
        XCTAssertEqual(ctx.prefix, "tod")
        XCTAssertEqual(ctx.from, 7)
    }

    func testCaretContextAfterCompletedPredicateIsKeyTier() {
        // A finished `key:value` plus trailing space — ready for the
        // next predicate (implicit AND).
        let ctx = QueryAuthoring.caretContext("status:todo ", cursor: 12)
        XCTAssertEqual(ctx.tier, .key)
        XCTAssertNil(ctx.key)
    }

    func testCaretContextAfterExplicitAndIsKeyTier() {
        let ctx = QueryAuthoring.caretContext("status:todo AND ", cursor: 16)
        XCTAssertEqual(ctx.tier, .key)
    }

    func testCaretContextInfixOperatorIsValueTier() {
        let ctx = QueryAuthoring.caretContext("priority >= ", cursor: 12)
        XCTAssertEqual(ctx.tier, .value)
        XCTAssertEqual(ctx.key, "priority")
    }

    func testCaretContextAfterInKeywordIsValueTier() {
        let ctx = QueryAuthoring.caretContext("type IN ", cursor: 8)
        XCTAssertEqual(ctx.tier, .value)
        XCTAssertEqual(ctx.key, "type")
    }

    func testCaretContextOpenParenAtStartIsKeyTier() {
        let ctx = QueryAuthoring.caretContext("(", cursor: 1)
        XCTAssertEqual(ctx.tier, .key)
    }

    func testCaretContextMidWordCaretIsNone() {
        // A caret with a non-whitespace character right after it (the
        // "true caret" contract this module supports even though every
        // current iOS call site always passes end-of-text) must not
        // interrupt mid-token editing.
        let ctx = QueryAuthoring.caretContext("status:todo", cursor: 3)
        XCTAssertEqual(ctx.tier, .none)
    }

    // MARK: - Completion candidate assembly (fake registry)

    private func fakeProperties() -> [String: PropertyDef] {
        let priority = PropertyDef(
            name: "Priority", valueType: .select, choices: ["P1", "P2", "P3"],
            def: nil, show: nil, hideByDefault: false, hideEmpty: true,
            chipIcon: nil, chipLabelMode: nil, chipShortLabel: nil, chipValueFormat: nil,
            chordKey: nil, valueChordKeys: [:], choiceColors: [:], nlTriggers: []
        )
        let notes = PropertyDef(
            name: "Notes", valueType: .text, choices: [],
            def: nil, show: nil, hideByDefault: false, hideEmpty: true,
            chipIcon: nil, chipLabelMode: nil, chipShortLabel: nil, chipValueFormat: nil,
            chordKey: nil, valueChordKeys: [:], choiceColors: [:], nlTriggers: []
        )
        return ["priority": priority, "notes": notes]
    }

    func testBuildCompletionsKeyTierIncludesPropertiesAndMetaKeys() {
        let ctx = QueryAuthoring.CaretContext(tier: .key, from: 0, to: 0, prefix: "", key: nil)
        let items = QueryAuthoring.buildCompletions(ctx, properties: fakeProperties(), typeNames: ["Task"])
        let labels = Set(items.map(\.label))
        XCTAssertTrue(labels.contains("Priority"))
        XCTAssertTrue(labels.contains("Notes"))
        for meta in QueryAuthoring.metaKeys {
            XCTAssertTrue(labels.contains(meta), "expected meta key \(meta)")
        }
    }

    func testBuildCompletionsOperatorTierReturnsFixedMenu() {
        let ctx = QueryAuthoring.CaretContext(tier: .operatorTier, from: 0, to: 0, prefix: "", key: "status")
        let items = QueryAuthoring.buildCompletions(ctx, properties: [:], typeNames: [])
        XCTAssertEqual(items.map(\.label), QueryAuthoring.operatorItems)
    }

    func testBuildCompletionsValueTierForSelectPropertyReturnsChoices() {
        let ctx = QueryAuthoring.CaretContext(tier: .value, from: 0, to: 0, prefix: "", key: "priority")
        let items = QueryAuthoring.buildCompletions(ctx, properties: fakeProperties(), typeNames: [])
        XCTAssertEqual(items.map(\.label), ["P1", "P2", "P3"])
    }

    func testBuildCompletionsValueTierForTypeKeyReturnsTypeNames() {
        let ctx = QueryAuthoring.CaretContext(tier: .value, from: 0, to: 0, prefix: "", key: "type")
        let items = QueryAuthoring.buildCompletions(ctx, properties: [:], typeNames: ["Task", "Project"])
        XCTAssertEqual(items.map(\.label), ["Task", "Project"])
    }

    func testBuildCompletionsValueTierForNonSelectPropertyReturnsEmpty() {
        let ctx = QueryAuthoring.CaretContext(tier: .value, from: 0, to: 0, prefix: "", key: "notes")
        let items = QueryAuthoring.buildCompletions(ctx, properties: fakeProperties(), typeNames: [])
        XCTAssertTrue(items.isEmpty)
    }

    func testBuildCompletionsNoneTierReturnsEmpty() {
        let ctx = QueryAuthoring.CaretContext(tier: .none, from: 0, to: 0, prefix: "", key: nil)
        XCTAssertTrue(QueryAuthoring.buildCompletions(ctx, properties: fakeProperties(), typeNames: ["Task"]).isEmpty)
    }

    // MARK: - Completion splice

    func testApplyCompletionKeyTierAppendsColon() {
        let ctx = QueryAuthoring.CaretContext(tier: .key, from: 0, to: 0, prefix: "", key: nil)
        let result = QueryAuthoring.applyCompletion("", ctx, "status")
        XCTAssertEqual(result.text, "status:")
        XCTAssertEqual(result.cursor, 7)
    }

    func testApplyCompletionOperatorTierAddsTrailingSpace() {
        let ctx = QueryAuthoring.CaretContext(tier: .operatorTier, from: 7, to: 7, prefix: "", key: "status")
        let result = QueryAuthoring.applyCompletion("status ", ctx, "!=")
        XCTAssertEqual(result.text, "status != ")
    }

    func testApplyCompletionOperatorTierColonHasNoTrailingSpace() {
        let ctx = QueryAuthoring.CaretContext(tier: .operatorTier, from: 7, to: 7, prefix: "", key: "status")
        let result = QueryAuthoring.applyCompletion("status ", ctx, ":")
        XCTAssertEqual(result.text, "status :")
    }

    func testApplyCompletionValueTierQuotesWhitespaceValues() {
        let ctx = QueryAuthoring.CaretContext(tier: .value, from: 5, to: 5, prefix: "", key: "tag")
        let result = QueryAuthoring.applyCompletion("tag:", ctx, "To Read")
        XCTAssertEqual(result.text, "tag:\"To Read\" ")
    }

    // MARK: - Chip registry: colon-DSL fragment ↔ JQL clause equivalence

    /// Every chip's legacy `clauses` fragment and its new `jqlClause`
    /// must parse to the SAME predicate (structural equality on the
    /// canonical form `QueryAuthoring.canonicalPredicate` computes) — the
    /// table `ChipDef.jqlClause`'s doc comment documents for tesela-vp9.3
    /// to mirror.
    func testChipRegistryFragmentsAndJqlClausesParseToEqualPredicates() {
        for chip in chipRegistry {
            let fragmentExpr = LocalQueryEngine.parseSimpleDsl(chip.clauses.joined(separator: " ")).expr
            let jqlExpr = LocalQueryEngine.parseSimpleDsl(chip.jqlClause).expr
            XCTAssertEqual(
                QueryAuthoring.canonicalPredicate(fragmentExpr),
                QueryAuthoring.canonicalPredicate(jqlExpr),
                "chip \(chip.id): '\(chip.clauses.joined(separator: " "))' and '\(chip.jqlClause)' must canonicalize equal"
            )
        }
    }

    // MARK: - Toggle round-trip (SavedViewLogic, parse-aware)

    func testToggleRoundTripOnActiveOffInactive() {
        for chip in chipRegistry {
            var dsl = "kind:block"
            XCTAssertFalse(
                SavedViewLogic.fragmentActive(chip.jqlClause, in: dsl),
                "chip \(chip.id) must start inactive"
            )
            dsl = SavedViewLogic.toggleFragment(chip.jqlClause, in: dsl)
            XCTAssertTrue(
                SavedViewLogic.fragmentActive(chip.jqlClause, in: dsl),
                "chip \(chip.id) must be active after toggle-on: \(dsl)"
            )
            dsl = SavedViewLogic.toggleFragment(chip.jqlClause, in: dsl)
            XCTAssertFalse(
                SavedViewLogic.fragmentActive(chip.jqlClause, in: dsl),
                "chip \(chip.id) must be inactive after toggle-off: \(dsl)"
            )
            XCTAssertEqual(dsl, "kind:block")
        }
    }

    func testToggleRoundTripPreservesHandTypedJqlAndLegacyForms() {
        // A hand-typed clause (in either legacy colon or full-JQL form)
        // that isn't the chip's own clause survives every toggle
        // untouched, and the LEGACY form the chip used to write is still
        // recognized as "active" (so an old saved view upgrades cleanly).
        let handTyped = "priority >= 3"
        var dsl = "kind:block \(handTyped) -has:status"

        // The legacy "-has:status" fragment the chip used to write is
        // recognized as active under the chip's NEW jql clause.
        let untriaged = chipRegistry.first { $0.id == "untriaged" }!
        XCTAssertTrue(SavedViewLogic.fragmentActive(untriaged.jqlClause, in: dsl))

        dsl = SavedViewLogic.toggleFragment(untriaged.jqlClause, in: dsl)
        XCTAssertFalse(SavedViewLogic.fragmentActive(untriaged.jqlClause, in: dsl))
        XCTAssertTrue(dsl.contains(handTyped), "hand-typed clause must survive: \(dsl)")

        dsl = SavedViewLogic.toggleFragment(untriaged.jqlClause, in: dsl)
        XCTAssertTrue(SavedViewLogic.fragmentActive(untriaged.jqlClause, in: dsl))
        XCTAssertTrue(dsl.contains(handTyped), "hand-typed clause must survive: \(dsl)")
    }

    func testToggleMultipleChipsIndependently() {
        var dsl = "kind:block"
        let untriaged = chipRegistry.first { $0.id == "untriaged" }!
        let notHeading = chipRegistry.first { $0.id == "notHeading" }!

        dsl = SavedViewLogic.toggleFragment(untriaged.jqlClause, in: dsl)
        dsl = SavedViewLogic.toggleFragment(notHeading.jqlClause, in: dsl)
        XCTAssertTrue(SavedViewLogic.fragmentActive(untriaged.jqlClause, in: dsl))
        XCTAssertTrue(SavedViewLogic.fragmentActive(notHeading.jqlClause, in: dsl))

        // Toggling one off leaves the other active.
        dsl = SavedViewLogic.toggleFragment(untriaged.jqlClause, in: dsl)
        XCTAssertFalse(SavedViewLogic.fragmentActive(untriaged.jqlClause, in: dsl))
        XCTAssertTrue(SavedViewLogic.fragmentActive(notHeading.jqlClause, in: dsl))
    }

    // MARK: - canonicalPredicate

    func testCanonicalPredicateFoldsNotEqIntoNe() {
        let notEq = LocalQueryEngine.SimpleDsl.BoolExpr.not(
            .atom(.cmp(key: "has", op: .eq, value: "status"))
        )
        XCTAssertEqual(
            QueryAuthoring.canonicalPredicate(notEq),
            .atom(.cmp(key: "has", op: .ne, value: "status"))
        )
    }

    func testCanonicalPredicateLeavesNonInvertibleUnchanged() {
        let expr = LocalQueryEngine.SimpleDsl.BoolExpr.atom(.cmp(key: "priority", op: .gte, value: "3"))
        XCTAssertEqual(QueryAuthoring.canonicalPredicate(expr), expr)
    }

    // MARK: - Real diagnostics in dslValidationError (tesela-vp9.5)

    func testValidationErrorUsesDiagnosticHintWhenAvailable() {
        // "hello world" — two dangling barewords, each recorded as a
        // diagnostic — the message must come from the FIRST diagnostic's
        // hint, not the generic fallback copy.
        let err = SavedViewLogic.dslValidationError("hello world")
        XCTAssertNotNil(err)
        XCTAssertTrue(err!.contains("hello"), "expected the diagnostic's 'got' snippet: \(err!)")
    }

    func testValidationErrorFallsBackToGenericMessageWithNoDiagnostics() {
        // Unrecognized bytes are dropped silently at the TOKENIZER level
        // (no diagnostic) — the generic zero-predicates message must
        // still fire.
        let err = SavedViewLogic.dslValidationError("???")
        XCTAssertNotNil(err)
        XCTAssertTrue(err!.contains("No filters recognized"), err!)
    }

    func testValidationErrorStillNilWhenAnyRealPredicateRecognized() {
        // A query with ONE real predicate plus dropped garbage stays
        // saveable — diagnostics only change the MESSAGE shown on an
        // already-invalid query, never invalidate a valid one.
        XCTAssertNil(SavedViewLogic.dslValidationError("status:todo blah"))
    }
}
