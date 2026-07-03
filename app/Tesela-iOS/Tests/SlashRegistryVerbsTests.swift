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

    // MARK: - P5.5 date-lift clear-intent gate + trailing-date rule (2026-06-30)

    func testNLPBareTrailingWeekdayLiftsDefaultDate() {
        // Taylor's locked decision: a bare weekday TRAILING the text (nothing
        // after it) lifts the type's default date property (Deadline for Task)
        // even with no intent word — matching how he types "p1 tomorrow".
        let text = "lets meet friday"
        let hit = InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                   tags: ["Task"], registry: registry, today: today)
        XCTAssertNotNil(hit, "a bare TRAILING weekday lifts the default date")
        if case .setProperty(let key, _)? = hit?.suggestion.action {
            XCTAssertEqual(key, "deadline", "Task's default/primary date prop is Deadline")
        } else {
            XCTFail("expected a deadline lift, got \(String(describing: hit?.suggestion.action))")
        }
    }

    func testNLPBareTrailingRelativeLiftsDefaultDate() {
        // Same for a bare relative token ("tomorrow") trailing the text.
        let text = "see you tomorrow"
        let hit = InlineNLP.detect(in: text, caretUTF16: (text as NSString).length,
                                   tags: ["Task"], registry: registry, today: today)
        XCTAssertNotNil(hit, "a bare TRAILING relative token lifts the default date")
        if case .setProperty(let key, let value)? = hit?.suggestion.action {
            XCTAssertEqual(key, "deadline")
            XCTAssertEqual(value, "2026-06-24")   // tomorrow
        } else {
            XCTFail("expected a deadline lift, got \(String(describing: hit?.suggestion.action))")
        }
    }

    func testNLPMidProseDateOffersNothing() {
        // SCOPE: the trailing-date rule is trailing-ONLY. A date with more words
        // after it is mid-prose and still needs an intent word → no date lift.
        // (Detecting at the caret after "tomorrow", with "about it" following.)
        let text = "call her tomorrow about it"
        let afterTomorrow = ("call her tomorrow" as NSString).length
        XCTAssertNil(InlineNLP.detect(in: text, caretUTF16: afterTomorrow,
                                      tags: ["Task"], registry: registry, today: today),
                     "a mid-prose date (more words follow) must not over-offer a lift")
    }

    // MARK: - Whole-block lifts: trailing date through detectLifts (capture path)

    func testDetectLiftsTrailingDateLiftsPriorityAndDeadline() {
        // "p1 tomorrow" on a #Task → priority p1 + Deadline tomorrow, text "".
        let r = InlineNLP.detectLifts(
            in: "p1 tomorrow", tags: ["#Task"], registry: registry, today: today)
        XCTAssertEqual(r.stripped, "")
        XCTAssertTrue(r.props.contains { $0.key == "priority" && $0.value == "p1" })
        XCTAssertTrue(r.props.contains { $0.key == "deadline" && $0.value == "2026-06-24" })
    }

    func testDetectLiftsShipItTrailingDate() {
        // "ship it p2 tomorrow" → "ship it" + p2 + Deadline tomorrow.
        let r = InlineNLP.detectLifts(
            in: "ship it p2 tomorrow", tags: ["#Task"], registry: registry, today: today)
        XCTAssertEqual(r.stripped, "ship it")
        XCTAssertTrue(r.props.contains { $0.key == "priority" && $0.value == "p2" })
        XCTAssertTrue(r.props.contains { $0.key == "deadline" && $0.value == "2026-06-24" })
    }

    func testDetectLiftsMidProseDateStaysProse() {
        // "call her tomorrow about p1" → priority p1 lifts; the mid-prose date
        // does NOT (stays in the text).
        let r = InlineNLP.detectLifts(
            in: "call her tomorrow about p1", tags: ["#Task"], registry: registry, today: today)
        XCTAssertTrue(r.props.contains { $0.key == "priority" && $0.value == "p1" })
        XCTAssertFalse(r.props.contains { $0.key == "deadline" })
        XCTAssertTrue(r.stripped.lowercased().contains("tomorrow"))
    }

    /// Block-editor parity with capture: on an UNSYNCED/empty live registry a
    /// `#Task` block lifts NOTHING when `detectLifts` resolves against the live
    /// registry directly (the build-62 block bug — the block path lacked
    /// capture's fallback), but lifts priority p2 + a deadline once it resolves
    /// through the SHARED `effectiveLiftRegistry` (the fix). Mirrors the capture
    /// regression `testCaptureWithTaskTypeTagsAndLiftsPropsOntoBlock`, but on
    /// the block lift path (`liftNlpOnBlur` / the highlight closure / slash
    /// verbs all now route through this helper).
    func testBlockDetectLiftsFallsBackToBuiltinsOnUnsyncedRegistry() {
        let unsynced = PropertyRegistry()  // empty: Property pages not yet synced
        XCTAssertFalse(
            unsynced.hasLiftableDefs(forTag: "Task"),
            "precondition: an empty live registry can't lift a #Task block")

        // WITHOUT the fallback (resolving against the live registry directly,
        // the old block behavior): nothing lifts, the prose is untouched.
        let without = InlineNLP.detectLifts(
            in: "Ship it p2 due tomorrow", tags: ["#Task"], registry: unsynced, today: today)
        XCTAssertTrue(
            without.props.isEmpty,
            "the unsynced live registry must lift nothing without the fallback")
        XCTAssertEqual(without.stripped, "Ship it p2 due tomorrow")

        // WITH the shared resolver: it falls back to the built-ins, so the block
        // lifts exactly like capture does.
        let reg = PropertyRegistry.effectiveLiftRegistry(live: unsynced, forTags: ["#Task"])
        let with = InlineNLP.detectLifts(
            in: "Ship it p2 due tomorrow", tags: ["#Task"], registry: reg, today: today)
        XCTAssertTrue(
            with.props.contains { $0.key == "priority" && $0.value == "p2" },
            "fallback must lift priority p2: \(with.props)")
        XCTAssertTrue(
            with.props.contains { $0.key == "deadline" && $0.value == "2026-06-24" },
            "fallback must lift the 'due tomorrow' deadline: \(with.props)")
        XCTAssertEqual(with.stripped, "Ship it")
    }

    /// `effectiveLiftRegistry` preserves precedence: a fully-synced (here the
    /// built-in) registry that CAN lift the tag is returned untouched — the
    /// fallback only fires for a non-liftable live registry, so user-customized
    /// type pages always win.
    func testEffectiveLiftRegistryPrefersLiveWhenLiftable() {
        let live = PropertyRegistry.buildBuiltins()  // can lift #Task
        XCTAssertTrue(live.hasLiftableDefs(forTag: "Task"))
        let reg = PropertyRegistry.effectiveLiftRegistry(live: live, forTags: ["#Task"])
        // Same liftable live registry → its lift result stands on its own.
        let r = InlineNLP.detectLifts(
            in: "ship it p2 tomorrow", tags: ["#Task"], registry: reg, today: today)
        XCTAssertTrue(r.props.contains { $0.key == "priority" && $0.value == "p2" })
        XCTAssertTrue(r.props.contains { $0.key == "deadline" && $0.value == "2026-06-24" })
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

    // MARK: - Live highlight spans (iOS surface parity)

    /// `detectHighlightRanges` returns the spans of every token that WOULD lift,
    /// in original-string coordinates — so the editor colors exactly what the
    /// commit-time lift will strip.
    func testHighlightRangesCoverPriorityAndDate() {
        let text = "Ship it p2 due tomorrow"
        let ranges = InlineNLP.detectHighlightRanges(
            in: text, tags: ["Task"], registry: registry, today: today)
        let ns = text as NSString
        let matched = Set(ranges.map { ns.substring(with: $0.range) })
        XCTAssertTrue(matched.contains("p2"), "p2 span highlighted: \(matched)")
        XCTAssertTrue(matched.contains("due tomorrow"), "date phrase highlighted: \(matched)")
        XCTAssertEqual(ranges.count, 2)
    }

    /// Semantic coloring (tesela-b1s): each highlight span carries its
    /// `HighlightKind` so the painter can color p2 yellow and the date phrase
    /// cyan instead of one flat accent.
    func testHighlightRangesCarrySemanticKind() {
        let text = "Ship it p2 due tomorrow"
        let spans = InlineNLP.detectHighlightRanges(
            in: text, tags: ["Task"], registry: registry, today: today)
        let ns = text as NSString
        for span in spans {
            let token = ns.substring(with: span.range)
            if token == "p2" {
                XCTAssertEqual(span.kind, .priority(2))
            } else if token == "due tomorrow" {
                XCTAssertEqual(span.kind, .date)
            } else {
                XCTFail("unexpected highlighted token: \(token)")
            }
        }
    }

    /// A bare TRAILING date now DOES lift (Taylor's locked decision), so it IS
    /// highlighted alongside the priority — highlight == lift, always.
    func testHighlightRangesBareTrailingDateHighlightsPriorityAndDate() {
        let text = "Ship it p2 tomorrow"
        let ranges = InlineNLP.detectHighlightRanges(
            in: text, tags: ["Task"], registry: registry, today: today)
        let ns = text as NSString
        let matched = Set(ranges.map { ns.substring(with: $0.range) })
        XCTAssertEqual(ranges.count, 2, "p2 + trailing tomorrow: \(matched)")
        XCTAssertTrue(matched.contains("p2"))
        XCTAssertTrue(matched.contains("tomorrow"))
    }

    /// Plain prose (no liftable tokens) / an untagged block highlights nothing.
    func testHighlightRangesEmptyForPlainAndUntagged() {
        XCTAssertTrue(InlineNLP.detectHighlightRanges(
            in: "just ordinary words", tags: ["Task"], registry: registry, today: today).isEmpty)
        XCTAssertTrue(InlineNLP.detectHighlightRanges(
            in: "Ship it p2 due tomorrow", tags: [], registry: registry, today: today).isEmpty)
    }
}
