import XCTest
@testable import Tesela

/// Phase 5.4 (registry-driven slash commands) + Phase 5.5 (inline NLP).
///
/// Locks two pure layers:
///   - `SlashVerbs.registryVerbs` / `matchingWithRegistry`: a tagged block's
///     select-choice + date verbs are generated from the RESOLVED registry
///     defs, routed to STRUCTURED actions (`.setProperty`/`.setStatus`/
///     `.openDateSheet`) — never `.insertText`. The format verbs survive.
///   - `InlineNLP.detect`: an exact `nl_trigger` token (`p1`) → a priority
///     lift; a confident date phrase (`due tomorrow`) → a date lift; plain
///     prose → nothing.
///
/// CONVERGENCE: the structured actions are what route the single
/// `commitSuggestion` dispatch to the typed per-key seam (`onSetProperty` →
/// `setBlockProperty`) instead of splicing property text into `text_seq`.
final class SlashRegistryVerbsTests: XCTestCase {

    /// The built-in registry mirrors the server seed: Task extends Root Tag
    /// with Status [todo,doing,done,blocked], Priority [p1..p4] (nl_triggers
    /// p1..p4), Deadline/Scheduled dates, Points number.
    private let registry = PropertyRegistry.buildBuiltins()

    // MARK: - P5.4 registry-verb generation

    func testRegistryVerbsForTaskIncludeStatusChoices() {
        let verbs = SlashVerbs.registryVerbs(tags: ["Task"], registry: registry)
        // A bare /<choice> verb per Task status choice, routed to .setStatus.
        for choice in ["todo", "doing", "done", "blocked"] {
            let hit = verbs.first { $0.label == choice && $0.action == .setStatus(choice: choice) }
            XCTAssertNotNil(hit, "missing /\(choice) status verb")
        }
        // A /<prop> <choice> form also present (e.g. "Status → doing").
        XCTAssertTrue(verbs.contains {
            $0.label == "Status → doing" && $0.action == .setStatus(choice: "doing")
        })
    }

    func testRegistryVerbsForTaskIncludePriorityChoices() {
        let verbs = SlashVerbs.registryVerbs(tags: ["Task"], registry: registry)
        // Priority is a non-status select → .setProperty(key:"priority").
        for p in ["p1", "p2", "p3", "p4"] {
            XCTAssertTrue(
                verbs.contains { $0.label == p && $0.action == .setProperty(key: "priority", value: p) },
                "missing /\(p) priority verb"
            )
        }
    }

    func testRegistryVerbsForTaskIncludeDateOpeners() {
        let verbs = SlashVerbs.registryVerbs(tags: ["Task"], registry: registry)
        XCTAssertTrue(verbs.contains { $0.action == .openDateSheet(field: .scheduled) })
        XCTAssertTrue(verbs.contains { $0.action == .openDateSheet(field: .deadline) })
    }

    func testRegistryVerbsRespectResolvedChoices() {
        // Project resolves Status to a DIFFERENT choice set than Task — the
        // verbs must come from the resolved def, not a global default.
        let verbs = SlashVerbs.registryVerbs(tags: ["Project"], registry: registry)
        let statusChoices = Set(verbs.compactMap { v -> String? in
            if case .setStatus(let c) = v.action { return c }
            return nil
        })
        XCTAssertEqual(statusChoices, ["planned", "active", "shipped"])
        // Task-only choices must NOT leak into a Project block.
        XCTAssertFalse(statusChoices.contains("blocked"))
    }

    func testRegistryVerbsEmptyForUntaggedBlock() {
        XCTAssertTrue(SlashVerbs.registryVerbs(tags: [], registry: registry).isEmpty)
    }

    func testMatchingWithRegistryMergesFormatAndPropertyVerbs() {
        let all = SlashVerbs.matchingWithRegistry("", tags: ["Task"], registry: registry)
        // Format verbs survive (link/tag openers + a heading).
        XCTAssertTrue(all.contains { $0.insert == "[[" && $0.action == .insertText })
        XCTAssertTrue(all.contains { $0.insert == "# " && $0.action == .insertText })
        // Registry verbs present.
        XCTAssertTrue(all.contains { $0.action == .setStatus(choice: "todo") })
        // Query filter narrows to the priority verbs.
        let p1 = SlashVerbs.matchingWithRegistry("p1", tags: ["Task"], registry: registry)
        XCTAssertTrue(p1.contains { $0.action == .setProperty(key: "priority", value: "p1") })
        XCTAssertFalse(p1.contains { $0.action == .setStatus(choice: "todo") })
    }

    // MARK: - P5.4 action-dispatch shape (structured, not text)

    func testStructuredVerbsCarryNoInsertText() {
        // The dispatch (commitSuggestion) splices ONLY for .insertText; every
        // registry-derived property verb must carry an empty insert + a
        // structured action so it can NEVER reach the text_seq splice path.
        let verbs = SlashVerbs.registryVerbs(tags: ["Task"], registry: registry)
        XCTAssertFalse(verbs.isEmpty)
        for v in verbs {
            XCTAssertEqual(v.insert, "", "structured verb \(v.id) must not carry insert text")
            XCTAssertNotEqual(v.action, .insertText, "verb \(v.id) must be a structured action")
        }
    }

    // MARK: - P5.5 inline NLP detection

    /// A fixed local "today" so date assertions are deterministic
    /// (2026-06-23 is a Tuesday). Built in the gregorian calendar's local zone
    /// to match `DateParser`'s `fmt`.
    private func fixed(_ y: Int, _ m: Int, _ d: Int) -> Date {
        var c = DateComponents(); c.year = y; c.month = m; c.day = d
        return Calendar(identifier: .gregorian).date(from: c)!
    }
    private var today: Date { fixed(2026, 6, 23) }

    func testNLPPriorityTokenLift() {
        // Typing "p1" on a Task surfaces a priority lift to the structured seam.
        let text = "ship it p1"
        let hit = InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                   tags: ["Task"], registry: registry, today: today)
        XCTAssertNotNil(hit)
        XCTAssertEqual(hit?.suggestion.action, .setProperty(key: "priority", value: "p1"))
        // The matched span is exactly the "p1" token (removed on apply).
        XCTAssertEqual(hit.map { (text as NSString).substring(with: NSRange(location: $0.start, length: $0.length)) }, "p1")
    }

    func testNLPDatePhraseLift() {
        // "due tomorrow" → a deadline date lift (DateParser confident parse).
        let text = "call mom due tomorrow"
        let hit = InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                   tags: ["Task"], registry: registry, today: today)
        XCTAssertNotNil(hit)
        if case .setProperty(let key, let value)? = hit?.suggestion.action {
            XCTAssertEqual(key, "deadline")
            XCTAssertEqual(value, "2026-06-24")   // tomorrow
        } else {
            XCTFail("expected a deadline setProperty lift, got \(String(describing: hit?.suggestion.action))")
        }
    }

    func testNLPScheduledPhraseLift() {
        // "scheduled friday" → a scheduled date lift (field from the phrase).
        let text = "review scheduled friday"
        let hit = InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                   tags: ["Task"], registry: registry, today: today)
        if case .setProperty(let key, _)? = hit?.suggestion.action {
            XCTAssertEqual(key, "scheduled")
        } else {
            XCTFail("expected a scheduled lift, got \(String(describing: hit?.suggestion.action))")
        }
    }

    func testNLPPlainProseOffersNothing() {
        let text = "just some ordinary words here"
        XCTAssertNil(InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                      tags: ["Task"], registry: registry, today: today))
    }

    func testNLPUntaggedBlockOffersNothing() {
        // No tags → no resolved defs → no lift even for "p1".
        let text = "p1"
        XCTAssertNil(InlineNLP.detect(in: text, caretUTF16: 2,
                                      tags: [], registry: registry, today: today))
    }

    func testNLPNonChoiceTokenIgnored() {
        // "p9" is not a priority choice/nl_trigger → no lift.
        let text = "todo p9"
        XCTAssertNil(InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                      tags: ["Task"], registry: registry, today: today))
    }

    // MARK: - P5.5 date-lift clear-intent gate

    func testNLPBareWeekdayMidSentenceOffersNothing() {
        // A bare weekday after an ordinary word is NOT clear date intent → no
        // lift, even though DateParser alone parses "friday".
        let text = "lets meet friday"
        XCTAssertNil(InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                      tags: ["Task"], registry: registry, today: today),
                     "a bare mid-sentence weekday must not over-offer a date lift")
    }

    func testNLPBareRelativeMidSentenceOffersNothing() {
        // Same for a bare relative token ("tomorrow") sitting mid-prose.
        let text = "see you tomorrow"
        XCTAssertNil(InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                      tags: ["Task"], registry: registry, today: today),
                     "a bare mid-sentence relative token must not over-offer a date lift")
    }

    func testNLPDatePrepositionGivesIntent() {
        // "on friday" → preceded by the date preposition "on" → a lift.
        let text = "standup on friday"
        let hit = InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                   tags: ["Task"], registry: registry, today: today)
        XCTAssertNotNil(hit, "a date preposition before a weekday should offer a lift")
        if case .setProperty(let key, _)? = hit?.suggestion.action {
            XCTAssertTrue(key == "scheduled" || key == "deadline")
        } else {
            XCTFail("expected a date setProperty lift, got \(String(describing: hit?.suggestion.action))")
        }
    }

    func testNLPDueRelativeGivesIntent() {
        // "due tomorrow" → the "due" keyword infers a deadline field → a lift.
        let text = "finish report due tomorrow"
        let hit = InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                   tags: ["Task"], registry: registry, today: today)
        XCTAssertNotNil(hit)
        if case .setProperty(let key, let value)? = hit?.suggestion.action {
            XCTAssertEqual(key, "deadline")
            XCTAssertEqual(value, "2026-06-24")
        } else {
            XCTFail("expected a deadline lift, got \(String(describing: hit?.suggestion.action))")
        }
    }

    func testNLPLineStartWeekdayGivesIntent() {
        // A weekday at the very line start IS clear intent → a lift.
        let text = "friday"
        let hit = InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                   tags: ["Task"], registry: registry, today: today)
        XCTAssertNotNil(hit, "a line-start weekday should offer a date lift")
        if case .setProperty(let key, _)? = hit?.suggestion.action {
            XCTAssertTrue(key == "scheduled" || key == "deadline")
        } else {
            XCTFail("expected a date setProperty lift, got \(String(describing: hit?.suggestion.action))")
        }
    }
}
