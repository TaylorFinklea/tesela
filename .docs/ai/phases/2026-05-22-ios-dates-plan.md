# First-Class Dates (iOS) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the iOS app to the First-Class Dates model the web ships — single "Date" keyboard-toolbar button opens an NL field + native picker sheet, dates render as inline chips on task blocks, `bareDateField` setting mirrors the web's.

**Architecture:** iOS already round-trips `key:: value` block properties (`Block.properties`), so this work is **purely additive UI** — no data migration. A Swift port of the web's `date-parser.ts` keeps NL parsing offline-capable and keeps the parsers in lockstep. The Date sheet writes properties via the existing `PUT /notes/{id}` save path (no new endpoint dependency).

**Tech Stack:** Swift / SwiftUI on iOS, XCTest, the existing iOS app at `app/Tesela-iOS/` (xcodegen-managed; `project.yml` is the source of truth, the `.xcodeproj` is generated).

**Reference spec:** `.docs/ai/phases/2026-05-22-ios-dates-design.md`
**Web parser to port:** `web/src/lib/date-parser.ts`
**Web tests to mirror:** `web/tests/unit/date-parser.test.mjs`

---

## File Structure

- `app/Tesela-iOS/project.yml` — **modify.** Add a new `TeselaTests` XCTest target (one-time setup).
- `app/Tesela-iOS/Sources/Data/DateParser.swift` — **create.** Swift port of the web NL parser.
- `app/Tesela-iOS/Tests/DateParserTests.swift` — **create.** XCTest cases mirroring `date-parser.test.mjs`.
- `app/Tesela-iOS/Sources/Data/DateFormat.swift` — **create.** Swift port of `formatDateMonthDay`.
- `app/Tesela-iOS/Tests/DateFormatTests.swift` — **create.** Tests for it.
- `app/Tesela-iOS/Sources/Data/BackendSettings.swift` — **modify.** Add `bareDateField` `@AppStorage`.
- `app/Tesela-iOS/Sources/Views/Settings/SettingsView.swift` — **modify.** Picker in Capture section.
- `app/Tesela-iOS/Sources/Components/TagChip.swift` — **modify.** Add `DeadlineChip` + `ScheduledChip` sibling views (mirrors `RecurrenceChip` already in this file).
- `app/Tesela-iOS/Sources/Components/BlockRow.swift` — **modify.** Mount the new chips in the existing chip `HStack`; consolidate `.deadline`/`.schedule` toolbar items into `.date`; wire `handleToolbarAction(.date)` to present the sheet; on sheet commit, upsert the property and persist via the existing edit path.
- `app/Tesela-iOS/Sources/Components/DateInputSheet.swift` — **create.** The sheet (NL field + DatePicker + recurrence row + skip button + Cancel/Set).
- `app/Tesela-iOS/Sources/Data/KeyboardToolbarItem.swift` — **modify.** Replace `.deadline` + `.schedule` cases with a single `.date` case.

---

## Task 1: Add an XCTest target (one-time infrastructure)

**Files:**
- Modify: `app/Tesela-iOS/project.yml`
- Create: `app/Tesela-iOS/Tests/SmokeTests.swift` (a single trivial passing test to prove the target works)

The iOS project has only one target (`Tesela`, type `application`) — Tasks 2 and 3 need an XCTest target to host their tests. This task sets it up once.

- [ ] **Step 1: Read the current `project.yml`**

`cat app/Tesela-iOS/project.yml` to see the existing structure (the `targets:` map). Note the bundle identifier convention, deployment target, and Swift version used by the `Tesela` target — the test target should match.

- [ ] **Step 2: Add the test target**

In `project.yml`'s `targets:` map, add (adapt deployment-target / Swift-version to whatever the `Tesela` target uses):

```yaml
  TeselaTests:
    type: bundle.unit-test
    platform: iOS
    sources:
      - path: Tests
    dependencies:
      - target: Tesela
    settings:
      base:
        BUNDLE_LOADER: $(TEST_HOST)
        TEST_HOST: $(BUILT_PRODUCTS_DIR)/Tesela.app/Tesela
        PRODUCT_BUNDLE_IDENTIFIER: com.tesela.ios.tests
```

(If the existing `Tesela` target sets `IPHONEOS_DEPLOYMENT_TARGET` / `SWIFT_VERSION` in its `settings.base`, copy those into the test target's `settings.base` too.)

Also add a scheme entry if `project.yml` defines schemes explicitly:

```yaml
schemes:
  Tesela:
    build:
      targets:
        Tesela: all
        TeselaTests: [test]
    test:
      targets:
        - TeselaTests
```

(Add only the `test:` block if the scheme already exists.)

- [ ] **Step 3: Create the smoke test**

Create `app/Tesela-iOS/Tests/SmokeTests.swift`:

```swift
import XCTest

final class SmokeTests: XCTestCase {
    func testSmoke() {
        XCTAssertEqual(1 + 1, 2)
    }
}
```

- [ ] **Step 4: Regenerate the xcodeproj**

```bash
cd /Users/tfinklea/git/tesela/app/Tesela-iOS && xcodegen generate
```

Confirm `xcodegen` reports no errors and the test target now appears in the generated project.

- [ ] **Step 5: Run the smoke test**

```bash
xcodebuild test \
  -project /Users/tfinklea/git/tesela/app/Tesela-iOS/Tesela-iOS.xcodeproj \
  -scheme Tesela \
  -destination 'platform=iOS Simulator,name=iPhone 15' \
  2>&1 | grep -E "Test Suite|Executed|TEST SUCCEEDED|TEST FAILED" | tail -10
```

(If the named simulator isn't installed, list available destinations via `xcrun simctl list devices available` and substitute.)
Expected: `Test Suite 'SmokeTests' passed. Executed 1 test, 0 failures` and `** TEST SUCCEEDED **`.

- [ ] **Step 6: Commit**

```bash
git add app/Tesela-iOS/project.yml app/Tesela-iOS/Tesela-iOS.xcodeproj app/Tesela-iOS/Tests/SmokeTests.swift
git commit -m "chore(ios): add XCTest target (TeselaTests) for upcoming parser tests"
```

(Stage the regenerated `.xcodeproj` if the repo commits it — `git status` after `xcodegen generate` will show whether the project file is tracked. If `Tesela-iOS.xcodeproj` is in `.gitignore`, drop it from the `git add`.)

---

## Task 2: Swift `DateParser` (port of `date-parser.ts`)

**Files:**
- Create: `app/Tesela-iOS/Sources/Data/DateParser.swift`
- Create: `app/Tesela-iOS/Tests/DateParserTests.swift`

A faithful Swift port of `web/src/lib/date-parser.ts` returning the same `{ date, time, recurrence, field }` shape. Tests mirror `web/tests/unit/date-parser.test.mjs` so the two parsers stay in lockstep.

**Read first:** the full current `web/src/lib/date-parser.ts` — every helper, regex, and switch case. The Swift port preserves grammar exactly.

- [ ] **Step 1: Define the types**

Create `app/Tesela-iOS/Sources/Data/DateParser.swift`:

```swift
import Foundation

/// Discriminator for which date property a parsed phrase targets.
enum DateField: String, Sendable, Equatable {
    case deadline
    case scheduled
}

/// Result of parsing a natural-language date phrase.
struct ParsedDateTimeRecurrence: Sendable, Equatable {
    /// `YYYY-MM-DD`.
    let date: String
    /// `HH:mm` (24-hour) if a time was parsed; otherwise nil.
    let time: String?
    /// Canonical recurrence string (e.g. `"weekly"`, `"every mon, wed, fri"`, `"weekly until 2026-12-31"`) or nil if no recurrence in the phrase.
    let recurrence: String?
    /// `deadline` / `scheduled` if a leading keyword (`deadline`/`scheduled`/`due`) routed the phrase; nil otherwise — the caller falls back to the `bareDateField` setting.
    let field: DateField?
}

enum DateParser {
    /// Parse a natural-language date phrase. Returns nil if no date+phrase combination matched. Mirrors `parseDateAndRecurrenceInput` in `web/src/lib/date-parser.ts`.
    static func parse(_ input: String, today: Date = Date()) -> ParsedDateTimeRecurrence? {
        // Step 1: lowercase + trim. Empty → nil.
        let raw = input.trimmingCharacters(in: .whitespaces).lowercased()
        guard !raw.isEmpty else { return nil }

        // Step 2: strip leading deadline/scheduled/due keyword.
        let (field, afterField) = extractField(raw)

        // Step 3: strip trailing recurrence tail (needs leading whitespace).
        let recExtracted = extractRecurrence(afterField)

        // Step 4: parse the remaining text as a date+optional-time.
        if let parsed = parseDateInput(recExtracted.rest, today: today) {
            return ParsedDateTimeRecurrence(date: parsed.date, time: parsed.time, recurrence: recExtracted.recurrence, field: field)
        }

        // Step 5: bare recurrence (no date) — anchor to today so the engine has something to bump.
        let bareRec = recExtracted.recurrence ?? parseRecurrenceInput(afterField)
        if let bareRec = bareRec {
            return ParsedDateTimeRecurrence(date: fmt(today), time: nil, recurrence: bareRec, field: field)
        }
        return nil
    }

    // The rest of the implementation goes here. See Step 3 for the full porting recipe.
}
```

(Mark `field` and the parser publicly accessible from within the module — `enum DateParser` with `static func parse` is the surface; helpers stay private to the file.)

- [ ] **Step 2: Write the failing tests**

Create `app/Tesela-iOS/Tests/DateParserTests.swift` mirroring `web/tests/unit/date-parser.test.mjs`:

```swift
import XCTest
@testable import Tesela

final class DateParserTests: XCTestCase {
    private func fixed(_ y: Int, _ m: Int, _ d: Int) -> Date {
        var c = DateComponents()
        c.year = y; c.month = m; c.day = d
        return Calendar(identifier: .gregorian).date(from: c)!
    }

    func testParsesIsoDate() {
        XCTAssertEqual(DateParser.parse("2026-05-22", today: fixed(2026, 5, 22))?.date, "2026-05-22")
    }

    func testTodayTomorrow() {
        XCTAssertEqual(DateParser.parse("today", today: fixed(2026, 5, 22))?.date, "2026-05-22")
        XCTAssertEqual(DateParser.parse("tomorrow", today: fixed(2026, 5, 22))?.date, "2026-05-23")
        XCTAssertEqual(DateParser.parse("yesterday", today: fixed(2026, 5, 22))?.date, "2026-05-21")
    }

    func testNextWeekday() {
        // Fri May 22 2026; "next fri" → Fri May 29
        XCTAssertEqual(DateParser.parse("next fri", today: fixed(2026, 5, 22))?.date, "2026-05-29")
        // "fri" (soonest) → today since today IS Fri
        XCTAssertEqual(DateParser.parse("fri", today: fixed(2026, 5, 22))?.date, "2026-05-22")
        XCTAssertEqual(DateParser.parse("mon", today: fixed(2026, 5, 22))?.date, "2026-05-25")
    }

    func testInNDays() {
        XCTAssertEqual(DateParser.parse("in 3 days", today: fixed(2026, 5, 22))?.date, "2026-05-25")
        XCTAssertEqual(DateParser.parse("3d", today: fixed(2026, 5, 22))?.date, "2026-05-25")
        XCTAssertEqual(DateParser.parse("2w", today: fixed(2026, 5, 22))?.date, "2026-06-05")
    }

    func testMonthDay() {
        XCTAssertEqual(DateParser.parse("may 23", today: fixed(2026, 5, 22))?.date, "2026-05-23")
        XCTAssertEqual(DateParser.parse("23 may", today: fixed(2026, 5, 22))?.date, "2026-05-23")
    }

    func testTime() {
        let r = DateParser.parse("fri at 10am", today: fixed(2026, 5, 22))
        XCTAssertEqual(r?.date, "2026-05-22")
        XCTAssertEqual(r?.time, "10:00")
        let r2 = DateParser.parse("tomorrow 14:30", today: fixed(2026, 5, 22))
        XCTAssertEqual(r2?.time, "14:30")
        XCTAssertEqual(DateParser.parse("today noon", today: fixed(2026, 5, 22))?.time, "12:00")
    }

    func testTrailingRecurrence() {
        XCTAssertEqual(DateParser.parse("fri weekly", today: fixed(2026, 5, 22))?.recurrence, "weekly")
        XCTAssertEqual(DateParser.parse("may 1 every 2 weeks", today: fixed(2026, 5, 22))?.recurrence, "every 2 weeks")
        XCTAssertEqual(DateParser.parse("fri every mon, wed, fri", today: fixed(2026, 5, 22))?.recurrence, "every mon, wed, fri")
    }

    func testEndClauses() {
        XCTAssertEqual(DateParser.parse("fri weekly until 2026-12-31", today: fixed(2026, 5, 22))?.recurrence, "weekly until 2026-12-31")
        XCTAssertEqual(DateParser.parse("fri weekly count 10", today: fixed(2026, 5, 22))?.recurrence, "weekly count 10")
        XCTAssertNil(DateParser.parse("fri weekly until 2026-02-30", today: fixed(2026, 5, 22))?.recurrence) // overflow rejected
    }

    func testFieldKeyword() {
        let fri = fixed(2026, 5, 22)
        XCTAssertEqual(DateParser.parse("deadline friday", today: fri)?.field, .deadline)
        XCTAssertEqual(DateParser.parse("scheduled tomorrow", today: fri)?.field, .scheduled)
        XCTAssertEqual(DateParser.parse("due may 1", today: fri)?.field, .deadline) // due → deadline
        XCTAssertNil(DateParser.parse("tomorrow", today: fri)?.field) // no keyword → nil
        let r = DateParser.parse("deadline every day", today: fri)
        XCTAssertEqual(r?.field, .deadline)
        XCTAssertEqual(r?.recurrence, "daily")
    }

    func testKeywordlessBareRecurrenceAnchorsToToday() {
        let fri = fixed(2026, 5, 22)
        let r = DateParser.parse("every monday", today: fri)
        XCTAssertEqual(r?.recurrence, "every mon")
        XCTAssertNil(r?.field)
        XCTAssertEqual(r?.date, "2026-05-22")
        XCTAssertEqual(DateParser.parse("weekdays", today: fri)?.recurrence, "weekdays")
        XCTAssertEqual(DateParser.parse("every 3 days", today: fri)?.recurrence, "every 3 days")
    }

    func testEmptyAndUnrecognized() {
        XCTAssertNil(DateParser.parse("", today: fixed(2026, 5, 22)))
        XCTAssertNil(DateParser.parse("not a date", today: fixed(2026, 5, 22)))
    }
}
```

- [ ] **Step 3: Run to verify failure**

```bash
xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 15' -only-testing:TeselaTests/DateParserTests 2>&1 | grep -E "Compile|error:|TEST FAILED" | head -20
```

Expected: compile errors (`DateParser` exists but the body is stubbed; helpers undefined). That's the failing-first state.

- [ ] **Step 4: Implement the parser body**

Port `date-parser.ts` to Swift. Below is the full implementation. **Read `date-parser.ts` first** to confirm each helper's exact behaviour matches; the port should be a translation, not a reinterpretation.

Add inside `enum DateParser` (or as file-private helpers below the enum):

```swift
private let WEEKDAYS: [String: Int] = [
    "sun": 0, "sunday": 0,
    "mon": 1, "monday": 1,
    "tue": 2, "tues": 2, "tuesday": 2,
    "wed": 3, "weds": 3, "wednesday": 3,
    "thu": 4, "thur": 4, "thurs": 4, "thursday": 4,
    "fri": 5, "friday": 5,
    "sat": 6, "saturday": 6,
]

private let MONTHS: [String: Int] = [
    "jan": 0, "january": 0, "feb": 1, "february": 1, "mar": 2, "march": 2,
    "apr": 3, "april": 3, "may": 4, "jun": 5, "june": 5, "jul": 6, "july": 6,
    "aug": 7, "august": 7, "sep": 8, "sept": 8, "september": 8,
    "oct": 9, "october": 9, "nov": 10, "november": 10, "dec": 11, "december": 11,
]

private let WEEKDAY_TOKENS: [String: String] = [
    "mon": "mon", "monday": "mon", "tue": "tue", "tues": "tue", "tuesday": "tue",
    "wed": "wed", "wednesday": "wed", "thu": "thu", "thur": "thu", "thurs": "thu", "thursday": "thu",
    "fri": "fri", "friday": "fri", "sat": "sat", "saturday": "sat", "sun": "sun", "sunday": "sun",
]
private let WEEKDAY_ORDER = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]

private let cal: Calendar = {
    var c = Calendar(identifier: .gregorian)
    c.timeZone = TimeZone.current
    return c
}()

private func fmt(_ d: Date) -> String {
    let comps = cal.dateComponents([.year, .month, .day], from: d)
    return String(format: "%04d-%02d-%02d", comps.year!, comps.month!, comps.day!)
}

private func addDays(_ base: Date, _ days: Int) -> Date {
    cal.date(byAdding: .day, value: days, to: base)!
}

private func makeDate(_ y: Int, _ m: Int, _ d: Int) -> Date? {
    var c = DateComponents(); c.year = y; c.month = m; c.day = d
    guard let dt = cal.date(from: c) else { return nil }
    let back = cal.dateComponents([.year, .month, .day], from: dt)
    guard back.year == y, back.month == m, back.day == d else { return nil }
    return dt
}

private func nextWeekday(_ base: Date, _ target: Int) -> Date {
    let cur = cal.component(.weekday, from: base) - 1 // Calendar weekday: 1=Sun → 0=Sun normalize
    let delta = ((target - cur + 7) % 7)
    return addDays(base, delta == 0 ? 7 : delta)
}

private func soonestWeekday(_ base: Date, _ target: Int) -> Date {
    let cur = cal.component(.weekday, from: base) - 1
    let delta = (target - cur + 7) % 7
    return addDays(base, delta)
}

private func extractField(_ raw: String) -> (DateField?, String) {
    // /^(deadline|scheduled|due)\s+(.+)$/
    let pattern = #"^(deadline|scheduled|due)\s+(.+)$"#
    if let m = raw.range(of: pattern, options: .regularExpression),
       m.lowerBound == raw.startIndex {
        let parts = raw[m]
        // Re-split to extract the two capture groups via NSRegularExpression.
        let re = try! NSRegularExpression(pattern: pattern)
        if let match = re.firstMatch(in: raw, range: NSRange(raw.startIndex..., in: raw)),
           match.numberOfRanges == 3,
           let kr = Range(match.range(at: 1), in: raw),
           let rr = Range(match.range(at: 2), in: raw) {
            let keyword = String(raw[kr])
            let rest = String(raw[rr])
            let field: DateField = (keyword == "due") ? .deadline : (keyword == "deadline" ? .deadline : .scheduled)
            return (field, rest)
        }
    }
    return (nil, raw)
}

// `extractTime`, `parseDatePart`, `parseDateInput`, `parseRecurrenceInput`,
// `parseRecurrenceFreq`, `extractRecurrence`, `TRAILING_RECUR_RE`:
// port each verbatim from `date-parser.ts` — same grammar, same edge cases.
// Use `NSRegularExpression` for the regexes; keep the canonical forms
// (e.g. `every mon, fri` sorted Mon-first) identical to the TS output.
//
// The full port is mechanical but ~300 LOC. Validate against the test
// suite — each grammar case in the tests must pass.
```

The implementer working this task does the full translation incrementally — port one helper at a time, run the affected tests after each, until all pass. The TS file is the source of truth; if behaviour differs, port matches TS, not the other way around. The `web/src/lib/date-parser.ts` final-review fix (`every monday` alone returns recurrence + today anchor with `field=nil`) must be in the port.

- [ ] **Step 5: Run tests until green**

```bash
xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 15' -only-testing:TeselaTests/DateParserTests 2>&1 | grep -E "Test Suite|Executed|failures|TEST" | tail -10
```

Expected: `Test Suite 'DateParserTests' passed. Executed N tests, 0 failures`.

- [ ] **Step 6: Commit**

```bash
git add app/Tesela-iOS/Sources/Data/DateParser.swift app/Tesela-iOS/Tests/DateParserTests.swift
git commit -m "feat(ios): DateParser — Swift port of web NL date parser"
```

---

## Task 3: Swift `DateFormat` (port of `formatDateMonthDay`)

**Files:**
- Create: `app/Tesela-iOS/Sources/Data/DateFormat.swift`
- Create: `app/Tesela-iOS/Tests/DateFormatTests.swift`

A Swift port of `web/src/lib/date-format.ts` — turns a property value (bare ISO `2026-05-25`, optional time, or legacy `[[..]]`-wrapped) into a human label.

- [ ] **Step 1: Write the failing tests**

Create `app/Tesela-iOS/Tests/DateFormatTests.swift`:

```swift
import XCTest
@testable import Tesela

final class DateFormatTests: XCTestCase {
    func testCurrentYearOmitsYear() {
        let thisYear = Calendar.current.component(.year, from: Date())
        XCTAssertEqual(DateFormat.humanMonthDay("\(thisYear)-05-22"), "May 22")
        XCTAssertEqual(DateFormat.humanMonthDay("[[\(thisYear)-05-22]]"), "May 22")
    }
    func testOtherYearIncludesYear() {
        XCTAssertEqual(DateFormat.humanMonthDay("2025-12-31"), "Dec 31, 2025")
    }
    func testTimeSuffix() {
        let thisYear = Calendar.current.component(.year, from: Date())
        XCTAssertEqual(DateFormat.humanMonthDay("\(thisYear)-05-22 15:30"), "May 22 3:30p")
        XCTAssertEqual(DateFormat.humanMonthDay("\(thisYear)-05-22 09:00"), "May 22 9a")
        XCTAssertEqual(DateFormat.humanMonthDay("\(thisYear)-05-22 12:00"), "May 22 12p")
    }
    func testUnrecognizedReturnsTrimmed() {
        XCTAssertEqual(DateFormat.humanMonthDay("not-a-date"), "not-a-date")
    }
}
```

- [ ] **Step 2: Run to verify failure**

```bash
xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 15' -only-testing:TeselaTests/DateFormatTests 2>&1 | grep -E "error:|TEST FAILED" | head -5
```

Expected: compile error (`DateFormat` undefined).

- [ ] **Step 3: Implement**

Create `app/Tesela-iOS/Sources/Data/DateFormat.swift`:

```swift
import Foundation

enum DateFormat {
    /// Human-readable rendering of a date property value. Accepts bare
    /// `YYYY-MM-DD` (optionally ` HH:mm`) or legacy `[[YYYY-MM-DD]]`.
    /// Unrecognized input returned trimmed-unchanged.
    /// Mirrors `web/src/lib/date-format.ts::formatDateMonthDay`.
    static func humanMonthDay(_ v: String) -> String {
        let trimmed = v.trimmingCharacters(in: .whitespaces)
        // Patterns:
        //   bracketed: [[YYYY-MM-DD]] or [[YYYY-MM-DD HH:MM]]
        //   bare:      YYYY-MM-DD or YYYY-MM-DD HH:MM
        let patterns = [
            #"^\[\[(\d{4})-(\d{2})-(\d{2})\]\](?:\s+(\d{2}):(\d{2}))?$"#,
            #"^(\d{4})-(\d{2})-(\d{2})(?:\s+(\d{2}):(\d{2}))?$"#,
        ]
        for p in patterns {
            let re = try! NSRegularExpression(pattern: p)
            if let m = re.firstMatch(in: trimmed, range: NSRange(trimmed.startIndex..., in: trimmed)),
               m.numberOfRanges >= 4,
               let yr = Range(m.range(at: 1), in: trimmed),
               let mr = Range(m.range(at: 2), in: trimmed),
               let dr = Range(m.range(at: 3), in: trimmed) {
                let y = Int(trimmed[yr])!, mo = Int(trimmed[mr])!, d = Int(trimmed[dr])!
                var dc = DateComponents(); dc.year = y; dc.month = mo; dc.day = d
                guard let date = Calendar(identifier: .gregorian).date(from: dc) else { return trimmed }
                let monthFmt = DateFormatter()
                monthFmt.locale = Locale(identifier: "en_US_POSIX")
                monthFmt.dateFormat = "MMM"
                let monthLabel = monthFmt.string(from: date)
                let thisYear = Calendar.current.component(.year, from: Date())
                let dateStr = y == thisYear ? "\(monthLabel) \(d)" : "\(monthLabel) \(d), \(y)"

                // Optional time
                if m.numberOfRanges >= 6,
                   let hr = Range(m.range(at: 4), in: trimmed),
                   let mr2 = Range(m.range(at: 5), in: trimmed) {
                    let hh = Int(trimmed[hr])!, mm = Int(trimmed[mr2])!
                    var h = hh; let ampm = h >= 12 ? "p" : "a"
                    h = h % 12 == 0 ? 12 : h % 12
                    let minStr = mm == 0 ? "" : String(format: ":%02d", mm)
                    return "\(dateStr) \(h)\(minStr)\(ampm)"
                }
                return dateStr
            }
        }
        return trimmed
    }
}
```

- [ ] **Step 4: Run tests until green**

```bash
xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 15' -only-testing:TeselaTests/DateFormatTests 2>&1 | grep -E "Test Suite|Executed|TEST" | tail -5
```

Expected: `Test Suite 'DateFormatTests' passed. Executed 4 tests, 0 failures`.

- [ ] **Step 5: Commit**

```bash
git add app/Tesela-iOS/Sources/Data/DateFormat.swift app/Tesela-iOS/Tests/DateFormatTests.swift
git commit -m "feat(ios): DateFormat.humanMonthDay — Swift port of formatDateMonthDay"
```

---

## Task 4: `bareDateField` setting

**Files:**
- Modify: `app/Tesela-iOS/Sources/Data/BackendSettings.swift`
- Modify: `app/Tesela-iOS/Sources/Views/Settings/SettingsView.swift`

A user preference selecting the default field for a date typed without a `deadline`/`scheduled`/`due` keyword.

- [ ] **Step 1: Read the current `BackendSettings.swift`**

`cat app/Tesela-iOS/Sources/Data/BackendSettings.swift` to see the existing `@AppStorage` patterns. Note the property-wrapper style and how the file exposes settings.

- [ ] **Step 2: Add the preference**

Append to the appropriate class/struct in `BackendSettings.swift`:

```swift
@AppStorage("bareDateField") var bareDateField: String = "scheduled"
```

(If `BackendSettings` is currently using `enum` keys or a different pattern, follow that convention. The `@AppStorage` default of `"scheduled"` mirrors web.)

- [ ] **Step 3: Add the picker in SettingsView**

Read `app/Tesela-iOS/Sources/Views/Settings/SettingsView.swift` lines 112-122 (the Capture section). Add inside that section, after the existing entries:

```swift
Picker("Default date field", selection: $backendSettings.bareDateField) {
    Text("Scheduled").tag("scheduled")
    Text("Deadline").tag("deadline")
}
```

(Adapt to the section's existing style — `Picker` inside a `Section { … }` block, matching the surrounding `Picker("Default target")` if there is one.)

- [ ] **Step 4: Build**

```bash
xcodebuild build -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'generic/platform=iOS Simulator' 2>&1 | grep -E "BUILD (SUCCEEDED|FAILED)|error:"
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 5: Commit**

```bash
git add app/Tesela-iOS/Sources/Data/BackendSettings.swift app/Tesela-iOS/Sources/Views/Settings/SettingsView.swift
git commit -m "feat(ios): bareDateField preference — default field for keyword-less dates"
```

---

## Task 5: `DeadlineChip` + `ScheduledChip` + mount in `BlockRow`

**Files:**
- Modify: `app/Tesela-iOS/Sources/Components/TagChip.swift`
- Modify: `app/Tesela-iOS/Sources/Components/BlockRow.swift`

Two display chips for a block's `deadline::` / `scheduled::` properties, rendered alongside the existing `TagChip` + `RecurrenceChip` in `BlockRow`'s chip `HStack`. Display-only here; tap-to-edit lands in Task 7 (after the sheet exists).

- [ ] **Step 1: Read prior art**

`cat app/Tesela-iOS/Sources/Components/TagChip.swift` (lines 50-68 are `RecurrenceChip` — the analog to mirror). Note: SF Symbol, monospaced font, muted tint background, capsule shape.

- [ ] **Step 2: Add `DeadlineChip` and `ScheduledChip` to `TagChip.swift`**

Append (mirroring `RecurrenceChip`'s style):

```swift
struct DeadlineChip: View {
    let value: String
    @Environment(\.theme) private var theme  // adapt to whatever the file uses for theming

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: "flag.fill")
                .font(.system(size: 9, weight: .medium))
            Text(DateFormat.humanMonthDay(value))
                .font(.system(size: 11, weight: .medium, design: .monospaced))
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 1)
        .foregroundColor(theme.fgMuted)
        .background(theme.fgMuted.opacity(0.10))
        .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}

struct ScheduledChip: View {
    let value: String
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: "calendar")
                .font(.system(size: 9, weight: .medium))
            Text(DateFormat.humanMonthDay(value))
                .font(.system(size: 11, weight: .medium, design: .monospaced))
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 1)
        .foregroundColor(theme.fgMuted)
        .background(theme.fgMuted.opacity(0.10))
        .clipShape(RoundedRectangle(cornerRadius: 3))
    }
}
```

Adapt the `@Environment(\.theme)` access to the file's real convention — if `RecurrenceChip` uses a different theme accessor, mirror it exactly. The SF symbols (`flag.fill`, `calendar`) are placeholders — pick whichever feels right with the existing aesthetic, but keep deadline and scheduled visually distinct.

- [ ] **Step 3: Mount in `BlockRow.swift`**

In `BlockRow.swift` around lines 62-92, read the existing chip `HStack` that renders tags + `RecurrenceChip` (line ~67-76). Find where `recurringValue` is computed; add siblings:

```swift
let deadlineValue: String? = block.properties.first(where: { $0.key == "deadline" })?.value
let scheduledValue: String? = block.properties.first(where: { $0.key == "scheduled" })?.value
```

Update the conditional guarding the chip `HStack` to also surface when either date is present:

```swift
if !tags.isEmpty || recurringValue != nil || deadlineValue != nil || scheduledValue != nil {
    HStack(spacing: 4) {
        ForEach(tags, id: \.self) { TagChip(tag: $0) }
        if let scheduledValue { ScheduledChip(value: scheduledValue) }
        if let deadlineValue { DeadlineChip(value: deadlineValue) }
        if let recurringValue { RecurrenceChip(value: recurringValue) }
    }
}
```

(Match the existing ordering and spacing conventions — read the current code first; the above is the target shape.)

- [ ] **Step 4: Build**

```bash
xcodebuild build -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'generic/platform=iOS Simulator' 2>&1 | grep -E "BUILD (SUCCEEDED|FAILED)|error:"
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 5: Commit**

```bash
git add app/Tesela-iOS/Sources/Components/TagChip.swift app/Tesela-iOS/Sources/Components/BlockRow.swift
git commit -m "feat(ios): DeadlineChip + ScheduledChip rendered on BlockRow"
```

---

## Task 6: `DateInputSheet.swift` — the picker sheet

**Files:**
- Create: `app/Tesela-iOS/Sources/Components/DateInputSheet.swift`

The sheet presented when the user taps the new "Date" keyboard-toolbar button. NL field + native `DatePicker` + recurrence row + Cancel/Set buttons.

- [ ] **Step 1: Skeleton — props + state**

Create `app/Tesela-iOS/Sources/Components/DateInputSheet.swift`:

```swift
import SwiftUI

/// Sheet for entering a date / recurrence on a block.
struct DateInputSheet: View {
    /// Pre-fill state from the block being edited (nil for a fresh entry).
    let initialScheduled: String?
    let initialDeadline: String?
    let initialRecurrence: String?
    /// Whether the underlying block already carries `recurring::` — used to gate the Skip button.
    let canSkip: Bool
    /// User's bareDateField default ("scheduled" / "deadline").
    let bareDateFieldDefault: String
    /// Called on Set with the resolved (field, isoDate, time?, recurrence?).
    let onCommit: (DateField, String, String?, String?) -> Void
    /// Called when the user taps Skip (only relevant if canSkip).
    let onSkip: () -> Void
    /// Called on Cancel or dismiss.
    let onCancel: () -> Void

    @State private var nlInput: String = ""
    @State private var pickedDate: Date = Date()
    @State private var pickedTime: Date? = nil
    @State private var pickedRecurrence: String? = nil
    @State private var resolvedField: DateField = .scheduled

    var body: some View {
        NavigationStack {
            Form {
                Section("Date") {
                    TextField("e.g. tomorrow, next fri, deadline may 23", text: $nlInput)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    DatePicker(
                        "Date",
                        selection: $pickedDate,
                        displayedComponents: [.date]
                    )
                    .datePickerStyle(.graphical)
                    Toggle("Set time", isOn: Binding(
                        get: { pickedTime != nil },
                        set: { pickedTime = $0 ? (pickedTime ?? Date()) : nil }
                    ))
                    if let timeBinding = Binding($pickedTime) {
                        DatePicker("Time", selection: timeBinding, displayedComponents: [.hourAndMinute])
                    }
                }
                Section("Repeat") {
                    Picker("Repeat", selection: Binding(
                        get: { pickedRecurrence ?? "none" },
                        set: { pickedRecurrence = $0 == "none" ? nil : $0 }
                    )) {
                        Text("None").tag("none")
                        Text("Daily").tag("daily")
                        Text("Weekdays").tag("weekdays")
                        Text("Weekly").tag("weekly")
                        Text("Monthly").tag("monthly")
                        Text("Yearly").tag("yearly")
                    }
                }
                Section {
                    HStack {
                        Text("Will set")
                        Spacer()
                        Text(resolvedField == .deadline ? "Deadline" : "Scheduled")
                            .foregroundStyle(.secondary)
                    }
                }
                if canSkip {
                    Section {
                        Button("Skip to next occurrence") { onSkip() }
                    }
                }
            }
            .navigationTitle("Date")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { onCancel() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Set") { commit() }
                }
            }
            .onChange(of: nlInput) { _, newValue in reparse(newValue) }
            .onAppear { seedFromInitial() }
        }
    }

    private func seedFromInitial() {
        // Prefer scheduled over deadline for the pre-fill date (mirrors the agenda's anchor order).
        let initialDate = initialScheduled ?? initialDeadline
        if let iso = initialDate, let d = parseIso(iso) { pickedDate = d }
        pickedRecurrence = initialRecurrence
        resolvedField = (initialDeadline != nil && initialScheduled == nil)
            ? .deadline
            : (bareDateFieldDefault == "deadline" ? .deadline : .scheduled)
    }

    private func reparse(_ s: String) {
        guard let parsed = DateParser.parse(s) else { return }
        if let d = parseIso(parsed.date) { pickedDate = d }
        if let t = parsed.time, let tm = parseTime(t) { pickedTime = tm }
        if let rec = parsed.recurrence { pickedRecurrence = rec }
        resolvedField = parsed.field ?? (bareDateFieldDefault == "deadline" ? .deadline : .scheduled)
    }

    private func commit() {
        let iso = isoString(pickedDate)
        let timeStr = pickedTime.map { timeString($0) }
        onCommit(resolvedField, iso, timeStr, pickedRecurrence)
    }

    private func parseIso(_ s: String) -> Date? {
        let f = DateFormatter(); f.dateFormat = "yyyy-MM-dd"; f.locale = Locale(identifier: "en_US_POSIX"); f.timeZone = TimeZone.current
        return f.date(from: s)
    }
    private func parseTime(_ s: String) -> Date? {
        let f = DateFormatter(); f.dateFormat = "HH:mm"; f.locale = Locale(identifier: "en_US_POSIX")
        return f.date(from: s)
    }
    private func isoString(_ d: Date) -> String {
        let f = DateFormatter(); f.dateFormat = "yyyy-MM-dd"; f.locale = Locale(identifier: "en_US_POSIX"); f.timeZone = TimeZone.current
        return f.string(from: d)
    }
    private func timeString(_ d: Date) -> String {
        let f = DateFormatter(); f.dateFormat = "HH:mm"; f.locale = Locale(identifier: "en_US_POSIX")
        return f.string(from: d)
    }
}
```

This is a fully self-contained sheet; the parent presents it via `.sheet(isPresented: …)` and supplies the callbacks. The implementer adapts the SwiftUI form to whatever idioms the existing iOS app uses (e.g. if the app uses a custom button/section style, follow it).

- [ ] **Step 2: Build**

```bash
xcodebuild build -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'generic/platform=iOS Simulator' 2>&1 | grep -E "BUILD (SUCCEEDED|FAILED)|error:"
```

Expected: `** BUILD SUCCEEDED **`. Sheet renders nothing without a presenter yet — Task 7 wires it.

- [ ] **Step 3: Commit**

```bash
git add app/Tesela-iOS/Sources/Components/DateInputSheet.swift
git commit -m "feat(ios): DateInputSheet — NL field + native picker + repeat + skip"
```

---

## Task 7: Toolbar wiring + commit path

**Files:**
- Modify: `app/Tesela-iOS/Sources/Data/KeyboardToolbarItem.swift`
- Modify: `app/Tesela-iOS/Sources/Components/BlockRow.swift`

Replace the two stubbed `.deadline` / `.schedule` cases with a single `.date` case; wire `BlockRow.handleToolbarAction(.date)` to present `DateInputSheet`; on `Set`, upsert the property/recurrence onto the block and persist via the existing edit path.

- [ ] **Step 1: Consolidate the keyboard-toolbar enum**

In `app/Tesela-iOS/Sources/Data/KeyboardToolbarItem.swift`:

- Remove the `.deadline` and `.schedule` cases (lines 14, 16 per the exploration).
- Add a single `.date` case.
- Update the icon/label switch arms (lines 31, 33, 47, 49) to a single `.date` arm:

```swift
case .date:
    // pick a fitting SF Symbol — `calendar.badge.plus` reads well for "add a date"
    "Date"  // label
    "calendar.badge.plus"  // icon
```

(Match the file's actual property/method shape — read it before editing; the above is what the values *should be*, formatted to fit the existing accessor functions.)

If `KeyboardToolbarItem` is persisted as a user-customisable list (via `@AppStorage` / a `defaultKeyboardToolbarItemsRaw` array as the exploration hinted), update that default list to replace `.deadline` and `.schedule` with `.date`.

- [ ] **Step 2: Wire the handler in `BlockRow.swift`**

In `BlockRow.swift` around lines 280-284 (the `handleToolbarAction` switch with the deadline/schedule stubs):

- Add SwiftUI state on the `BlockRow` view (top of the struct):

```swift
@State private var showingDateSheet = false
@AppStorage("bareDateField") private var bareDateFieldRaw: String = "scheduled"
```

- Replace the stub cases with:

```swift
case .date:
    showingDateSheet = true
```

- Add the sheet presentation at the bottom of the view body (after the chip `HStack`):

```swift
.sheet(isPresented: $showingDateSheet) {
    DateInputSheet(
        initialScheduled: block.properties.first(where: { $0.key == "scheduled" })?.value,
        initialDeadline: block.properties.first(where: { $0.key == "deadline" })?.value,
        initialRecurrence: block.properties.first(where: { $0.key == "recurring" })?.value,
        canSkip: block.properties.contains(where: { $0.key == "recurring" }),
        bareDateFieldDefault: bareDateFieldRaw,
        onCommit: { field, iso, time, recurrence in
            commitDate(field: field, iso: iso, time: time, recurrence: recurrence)
            showingDateSheet = false
        },
        onSkip: {
            Task { await skipRecurringOnBlock() }
            showingDateSheet = false
        },
        onCancel: { showingDateSheet = false }
    )
}
```

- Add the commit + skip functions to the view:

```swift
private func commitDate(field: DateField, iso: String, time: String?, recurrence: String?) {
    // Build the value: "YYYY-MM-DD" or "YYYY-MM-DD HH:mm".
    let value = time.map { "\(iso) \($0)" } ?? iso
    let key = field.rawValue  // "deadline" / "scheduled"

    // Upsert the property in `block.properties`, removing the other field if `field` is set
    // (a Date sheet commits to ONE field at a time — keep the other intact if it was already set).
    var updated = block.properties.filter { $0.key != key }  // drop any prior value at this key
    updated.append(BlockProperty(key: key, value: value))

    // Recurrence: upsert or leave as-is when nil.
    if let recurrence {
        updated.removeAll { $0.key == "recurring" }
        updated.append(BlockProperty(key: "recurring", value: recurrence))
    }

    // Hand the new properties to the parent — `BlockRow` is rendered with a
    // `block` binding (or a service call). The existing `editTodayBlock` /
    // `editPageBlock` save path takes a new `text` string; here we mutate
    // properties only, so call whichever existing method the parent uses
    // for property updates. If only text-based save exists, render the
    // updated block (text + properties) via the service's render path and
    // call the same edit method.

    // Implementation note: read `MockMosaicService.editTodayBlock` / `editPageBlock`
    // and any helpers nearby. If there's no per-property edit method, add one:
    //   func setBlockProperty(blockId: String, key: String, value: String) async throws
    // that mutates `block.properties` for the matching id in the snapshot and
    // calls `scheduleWriteback()` / `pushPage()`. Keep symmetry with `editTodayBlock`.
    Task { await mosaic.setBlockProperties(id: block.id, properties: updated) }
}

private func skipRecurringOnBlock() async {
    // Hit POST /blocks/recur-bump with mode "skip" — mirrors the web skip verb.
    do { try await mosaic.recurBump(blockId: block.id, mode: .skip) } catch { /* surface error */ }
}
```

`mosaic` is the service injected into `BlockRow` (likely `@EnvironmentObject` or a passed binding — match the file's existing convention).

If `MockMosaicService` does not yet expose `setBlockProperties(id:properties:)` or `recurBump(blockId:mode:)`, add them in the same task — they're tiny:

```swift
// In MockMosaicService.swift:
func setBlockProperties(id: String, properties: [BlockProperty]) async {
    // 1. Find block in todayBlocks or any open page, mutate its `.properties`.
    // 2. Call scheduleWriteback() (today) or pushPage(id:blocks:) (page).
}

enum RecurBumpMode: String { case complete, skip }

func recurBump(blockId: String, mode: RecurBumpMode) async throws {
    guard case .http(let baseURL) = currentBackend else { return }
    try await httpPost(
        "/blocks/recur-bump",
        baseURL: baseURL,
        body: ["block_id": blockId, "mode": mode.rawValue]
    )
    // Refresh affected note from server so the agenda/today view sees the bumped state.
}
```

- [ ] **Step 3: Make the chips tappable (re-opens the sheet)**

In `BlockRow.swift`'s chip `HStack`, wrap each new chip in a button:

```swift
if let scheduledValue {
    Button { showingDateSheet = true } label: {
        ScheduledChip(value: scheduledValue)
    }
    .buttonStyle(.plain)
}
// same for deadlineValue, recurringValue
```

(The `.plain` style avoids the default button border so the chip still reads as a chip.)

- [ ] **Step 4: Build + smoke**

```bash
xcodebuild build -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'generic/platform=iOS Simulator' 2>&1 | grep -E "BUILD (SUCCEEDED|FAILED)|error:"
```

Expected: `** BUILD SUCCEEDED **`.

Manually: launch the app in the simulator, open a task block, tap the new "Date" toolbar button → sheet opens → type "tomorrow", tap Set → the block now shows a `ScheduledChip` with the new date. Tap the chip → sheet re-opens pre-filled. Type "deadline next fri", tap Set → chip switches to `DeadlineChip`.

- [ ] **Step 5: Commit**

```bash
git add app/Tesela-iOS/Sources/Data/KeyboardToolbarItem.swift app/Tesela-iOS/Sources/Components/BlockRow.swift app/Tesela-iOS/Sources/Data/MockMosaicService.swift
git commit -m "feat(ios): wire Date keyboard-toolbar button — sheet + commit + skip"
```

(Stage `MockMosaicService.swift` only if you added `setBlockProperties` / `recurBump` to it.)

---

## Self-Review

**Spec coverage (design spec §1–7):**
- §1 data model — no migration needed (iOS already round-trips properties). Task 1 (KeyboardToolbarItem consolidation) and Task 7 (the actual write path) address it.
- §2 Swift NL parser — Task 2.
- §3 the Date input sheet — Task 6.
- §4 display chips (inline on `BlockRow`) — Tasks 3 + 5.
- §5 save path (existing `PUT /notes/{id}`) — Task 7 (`commitDate`).
- §6 `bareDateField` setting — Task 4.
- §7 out of scope — explicitly not in any task (capture-bar NL input, drag-reschedule, per-occurrence overrides, agenda).

**Type consistency:** `DateField = .deadline | .scheduled` defined in Task 2, consumed by `DateInputSheet.onCommit` (Task 6) and `BlockRow.commitDate` (Task 7). `ParsedDateTimeRecurrence` shape (`date`/`time`/`recurrence`/`field`) consistent across Tasks 2 and 6 (`reparse`). `DateFormat.humanMonthDay` (Task 3) consumed by `DeadlineChip` + `ScheduledChip` (Task 5). `bareDateField` `@AppStorage` key `"bareDateField"` used in Task 4 and read in Task 7's `BlockRow`.

**Placeholder scan:** Tasks 6 and 7 contain "read the file and mirror the existing convention" steps for SwiftUI styling and the `MockMosaicService` interface — those are intentional (local conventions can't be specified in advance from outside the file). All code-bearing steps carry real code. The Swift parser port (Task 2 Step 4) intentionally trails into "the rest is a mechanical translation" because reproducing 300+ LOC of port verbatim in the plan would bury the bite-sized steps for everything else — the TDD test suite is the gating spec, not the listing.

**Ordering:** Task 1 (test target) → Task 2 (parser, requires test target) → Task 3 (formatter, requires test target) → Task 4 (settings, used by Task 7) → Task 5 (chips, requires Task 3's formatter) → Task 6 (sheet, requires Task 2's parser + Task 4's setting) → Task 7 (wiring, requires Tasks 5 + 6 + 4). Linear dependency chain; no out-of-order surprises.
