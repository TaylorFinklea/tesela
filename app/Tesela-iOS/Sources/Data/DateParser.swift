import Foundation

// MARK: - Types

enum DateField: String, Sendable, Equatable {
    case deadline
    case scheduled
}

struct ParsedDateTimeRecurrence: Sendable, Equatable {
    let date: String        // YYYY-MM-DD
    let time: String?       // HH:mm (24-hour) or nil
    let recurrence: String?
    let field: DateField?
}

// MARK: - Lookup Tables

/// Weekday name → 0-based index (Sun=0 … Sat=6), matching JS Date.getDay()
private let WEEKDAYS: [String: Int] = [
    "sun": 0, "sunday": 0,
    "mon": 1, "monday": 1,
    "tue": 2, "tues": 2, "tuesday": 2,
    "wed": 3, "weds": 3, "wednesday": 3,
    "thu": 4, "thur": 4, "thurs": 4, "thursday": 4,
    "fri": 5, "friday": 5,
    "sat": 6, "saturday": 6,
]

/// Month name → 0-based month index (Jan=0 … Dec=11)
private let MONTHS: [String: Int] = [
    "jan": 0, "january": 0,
    "feb": 1, "february": 1,
    "mar": 2, "march": 2,
    "apr": 3, "april": 3,
    "may": 4,
    "jun": 5, "june": 5,
    "jul": 6, "july": 6,
    "aug": 7, "august": 7,
    "sep": 8, "sept": 8, "september": 8,
    "oct": 9, "october": 9,
    "nov": 10, "november": 10,
    "dec": 11, "december": 11,
]

// MARK: - Regex Patterns

/// Trailing recurrence regex: requires leading whitespace so it only matches after a date prefix.
private let TRAILING_RECUR_RE: NSRegularExpression = {
    let bydayToken = "(?:mon(?:day)?|tues?(?:day)?|wed(?:nesday)?|thu(?:rs?(?:day)?)?|fri(?:day)?|sat(?:urday)?|sun(?:day)?)"
    let bydaySet = "every\\s+\(bydayToken)(?:\\s*,\\s*\(bydayToken))*"
    let endClause = "(?:\\s+until\\s+\\d{4}-\\d{2}-\\d{2}|\\s+count\\s+\\d+)?"
    let pattern = "\\s+((?:daily|weekly|monthly|yearly|annually|biweekly|fortnightly|quarterly|weekdays|weekends|every\\s+other\\s+(?:days?|weeks?|months?|years?)|every\\s+\\d+\\s+(?:days?|weeks?|months?|years?)|every\\s+weekdays?|every\\s+(?:day|week|month|year)|\(bydaySet))\(endClause))$"
    return try! NSRegularExpression(pattern: pattern, options: .caseInsensitive)
}()

/// Trailing time regex
private let TRAILING_TIME_RE = try! NSRegularExpression(
    pattern: "(?:^|\\s)(at\\s+)?(\\d{1,2})(?::(\\d{2}))?\\s*(am|pm)?$",
    options: .caseInsensitive
)

// MARK: - Calendar Helpers

private let gregorian = Calendar(identifier: .gregorian)

/// Format a Date as YYYY-MM-DD
func fmt(_ d: Date) -> String {
    let c = gregorian.dateComponents([.year, .month, .day], from: d)
    let y = c.year!
    let m = String(c.month!).leftPad(2)
    let day = String(c.day!).leftPad(2)
    return "\(y)-\(m)-\(day)"
}

/// Add days to a Date
private func addDays(_ base: Date, _ days: Int) -> Date {
    gregorian.date(byAdding: .day, value: days, to: base)!
}

/// Construct a date from year/month/day (0-based month, matching JS Date(y, m, d)).
/// Returns nil if the resulting date doesn't round-trip (detects overflow like Feb 30).
private func makeDate(_ y: Int, _ m: Int, _ d: Int) -> Date? {
    var c = DateComponents()
    c.year = y
    c.month = m + 1  // JS uses 0-based month; Swift Calendar uses 1-based
    c.day = d
    guard let dt = gregorian.date(from: c) else { return nil }
    let rc = gregorian.dateComponents([.year, .month, .day], from: dt)
    guard rc.year == y, rc.month == m + 1, rc.day == d else { return nil }
    return dt
}

/// Next occurrence of the given weekday (1–7 days ahead — never today).
/// target is 0-based (Sun=0).
private func nextWeekday(_ base: Date, _ target: Int) -> Date {
    let cur = weekdayIndex(base)
    let delta = ((target - cur + 7) % 7 == 0) ? 7 : ((target - cur + 7) % 7)
    return addDays(base, delta)
}

/// Soonest occurrence of the given weekday (today if today matches, else upcoming).
/// target is 0-based (Sun=0).
private func soonestWeekday(_ base: Date, _ target: Int) -> Date {
    let cur = weekdayIndex(base)
    let delta = (target - cur + 7) % 7
    return addDays(base, delta)
}

/// Return the 0-based weekday index (Sun=0 … Sat=6), matching JS Date.getDay()
private func weekdayIndex(_ d: Date) -> Int {
    // Swift Calendar weekday: 1=Sun, 2=Mon, … 7=Sat → subtract 1 for 0-based
    gregorian.component(.weekday, from: d) - 1
}

// MARK: - String Helper

private extension String {
    func leftPad(_ width: Int) -> String {
        guard count < width else { return self }
        return String(repeating: "0", count: width - count) + self
    }
}

// MARK: - NSRegularExpression Helpers

private extension NSRegularExpression {
    func firstMatch(in s: String) -> NSTextCheckingResult? {
        firstMatch(in: s, range: NSRange(s.startIndex..., in: s))
    }
    func group(_ idx: Int, in result: NSTextCheckingResult, source: String) -> String? {
        let r = result.range(at: idx)
        guard r.location != NSNotFound, let range = Range(r, in: source) else { return nil }
        return String(source[range])
    }
}

// MARK: - Recurrence Parsing

/// Recognize + canonicalize a recurrence phrase. Delegates to the Rust FFI
/// (`tesela_core::recurrence::recognize`, exposed as `parseRecurrence`) —
/// the standalone Swift mirror that used to live here was deleted in
/// tesela-pfix.2. Returns the canonical storage string, or `nil` if
/// unrecognized.
func parseRecurrenceInput(_ input: String) -> String? {
    parseRecurrence(input: input)
}

// MARK: - Recurrence Extraction

/// Strip a trailing recurrence phrase from a date+recurrence string.
/// Requires leading whitespace — won't match bare phrases.
private func extractRecurrence(_ s: String) -> (recurrence: String?, rest: String) {
    guard let m = TRAILING_RECUR_RE.firstMatch(in: s),
          let tail = TRAILING_RECUR_RE.group(1, in: m, source: s) else {
        return (nil, s)
    }
    let tailLower = tail.lowercased()
    guard let rec = parseRecurrenceInput(tailLower) else { return (nil, s) }
    let matchRange = Range(m.range, in: s)!
    let restEnd = matchRange.lowerBound
    let rest = String(s[s.startIndex..<restEnd]).trimmingCharacters(in: .whitespaces)
    return (rec, rest)
}

// MARK: - Field Extraction

private let FIELD_RE = try! NSRegularExpression(pattern: "^(deadline|scheduled|due)\\s+(.+)$")

/// Strip a leading `deadline`/`scheduled`/`due` keyword.
private func extractField(_ raw: String) -> (field: DateField?, rest: String) {
    guard let m = FIELD_RE.firstMatch(in: raw),
          let keyword = FIELD_RE.group(1, in: m, source: raw),
          let rest = FIELD_RE.group(2, in: m, source: raw) else {
        return (nil, raw)
    }
    let field: DateField = keyword == "due" ? .deadline : (keyword == "deadline" ? .deadline : .scheduled)
    return (field, rest)
}

// MARK: - Time Extraction

private func extractTime(_ s: String) -> (time: String?, rest: String) {
    if s == "noon"     { return ("12:00", "today") }
    if s == "midnight" { return ("00:00", "today") }

    // Handle "noon" / "midnight" as trailing words (e.g. "today noon")
    if s.hasSuffix(" noon") {
        let rest = String(s.dropLast(5)).trimmingCharacters(in: .whitespaces)
        return ("12:00", rest.isEmpty ? "today" : rest)
    }
    if s.hasSuffix(" midnight") {
        let rest = String(s.dropLast(9)).trimmingCharacters(in: .whitespaces)
        return ("00:00", rest.isEmpty ? "today" : rest)
    }

    guard let m = TRAILING_TIME_RE.firstMatch(in: s) else { return (nil, s) }

    let hasAt    = TRAILING_TIME_RE.group(1, in: m, source: s) != nil
    let hourStr  = TRAILING_TIME_RE.group(2, in: m, source: s)
    let minStr   = TRAILING_TIME_RE.group(3, in: m, source: s)
    let ampmStr  = TRAILING_TIME_RE.group(4, in: m, source: s)?.lowercased()

    let hasColon = minStr != nil
    let hasAmPm  = ampmStr != nil
    guard hasAt || hasColon || hasAmPm else { return (nil, s) }

    guard var h = hourStr.flatMap({ Int($0) }) else { return (nil, s) }
    let mins = minStr.flatMap({ Int($0) }) ?? 0
    if ampmStr == "pm" && h < 12 { h += 12 }
    if ampmStr == "am" && h == 12 { h = 0 }
    guard h >= 0, h <= 23, mins >= 0, mins <= 59 else { return (nil, s) }

    let time = "\(String(h).leftPad(2)):\(String(mins).leftPad(2))"

    // Strip the matched portion from the end of s
    let matchRange = Range(m.range, in: s)!
    var restStr = String(s[s.startIndex..<matchRange.lowerBound])
    // If the match started with a space (group 0 starts with whitespace), the lowerBound
    // already excludes it, but we need to handle the case where the full match starts
    // at position 0 (time-only input like "10am").
    let fullMatch = String(s[matchRange])
    if fullMatch.hasPrefix(" ") {
        // rest is everything before the space — already correct
    }
    restStr = restStr.trimmingCharacters(in: .whitespaces)
    if restStr.isEmpty { restStr = "today" }
    return (time, restStr)
}

// MARK: - Date Part Parsing

private func parseDatePart(_ s: String, today: Date) -> String? {
    if s == "today" || s == "tod" { return fmt(today) }
    if s == "tomorrow" || s == "tom" || s == "tmrw" { return fmt(addDays(today, 1)) }
    if s == "yesterday" || s == "yes" || s == "yest" { return fmt(addDays(today, -1)) }

    // ISO: YYYY-MM-DD
    let isoRe = try! NSRegularExpression(pattern: "^(\\d{4})-(\\d{1,2})-(\\d{1,2})$")
    if let m = isoRe.firstMatch(in: s),
       let yStr = isoRe.group(1, in: m, source: s),
       let moStr = isoRe.group(2, in: m, source: s),
       let dStr = isoRe.group(3, in: m, source: s),
       let y = Int(yStr), let mo = Int(moStr), let d = Int(dStr) {
        return makeDate(y, mo - 1, d).map { fmt($0) }
    }

    // Slash: M/D or M/D/YY or M/D/YYYY
    let slashRe = try! NSRegularExpression(pattern: "^(\\d{1,2})/(\\d{1,2})(?:/(\\d{2}|\\d{4}))?$")
    if let m = slashRe.firstMatch(in: s),
       let moStr = slashRe.group(1, in: m, source: s),
       let dStr  = slashRe.group(2, in: m, source: s),
       let mo = Int(moStr), let d = Int(dStr) {
        let yrStr = slashRe.group(3, in: m, source: s)
        var year: Int
        if let ys = yrStr, let y = Int(ys) {
            year = y < 100 ? 2000 + y : y
        } else {
            let todayComps = gregorian.dateComponents([.year, .month, .day], from: today)
            year = todayComps.year!
            let todayStart = gregorian.date(from: DateComponents(year: todayComps.year, month: todayComps.month, day: todayComps.day))!
            if let candidate = makeDate(year, mo - 1, d), candidate < todayStart {
                year += 1
            }
        }
        return makeDate(year, mo - 1, d).map { fmt($0) }
    }

    // "apr 15" / "april 15" / "15 apr" — optional year
    // Regex: ^(?:(\d{1,2})\s+)?([a-z]+)(?:\s+(\d{1,2}))?(?:[,\s]+(\d{2}|\d{4}))?$
    let monthNameRe = try! NSRegularExpression(
        pattern: "^(?:(\\d{1,2})\\s+)?([a-z]+)(?:\\s+(\\d{1,2}))?(?:[,\\s]+(\\d{2}|\\d{4}))?$"
    )
    if let m = monthNameRe.firstMatch(in: s),
       let monthWord = monthNameRe.group(2, in: m, source: s),
       let mo = MONTHS[monthWord] {
        let dayBefore = monthNameRe.group(1, in: m, source: s)
        let dayAfter  = monthNameRe.group(3, in: m, source: s)
        let yearStr   = monthNameRe.group(4, in: m, source: s)
        let dayStr = dayBefore ?? dayAfter
        if let dayStr = dayStr, let d = Int(dayStr) {
            var year: Int
            if let ys = yearStr, let y = Int(ys) {
                year = y < 100 ? 2000 + y : y
            } else {
                let todayComps = gregorian.dateComponents([.year, .month, .day], from: today)
                year = todayComps.year!
                let todayStart = gregorian.date(from: DateComponents(year: todayComps.year, month: todayComps.month, day: todayComps.day))!
                if let candidate = makeDate(year, mo, d), candidate < todayStart {
                    year += 1
                }
            }
            return makeDate(year, mo, d).map { fmt($0) }
        }
    }

    if s == "next week" || s == "nw" { return fmt(addDays(today, 7)) }

    // "next <weekday>"
    let nextWdRe = try! NSRegularExpression(pattern: "^next\\s+([a-z]+)$")
    if let m = nextWdRe.firstMatch(in: s),
       let wdName = nextWdRe.group(1, in: m, source: s),
       let wd = WEEKDAYS[wdName] {
        return fmt(nextWeekday(today, wd))
    }

    // Bare weekday
    if let wd = WEEKDAYS[s] { return fmt(soonestWeekday(today, wd)) }

    // "in N days/weeks/months"
    let inNRe = try! NSRegularExpression(pattern: "^in\\s+(\\d+)\\s+(day|days|week|weeks|month|months|d|w|mo)$")
    if let m = inNRe.firstMatch(in: s),
       let nStr = inNRe.group(1, in: m, source: s),
       let unit = inNRe.group(2, in: m, source: s),
       let n = Int(nStr) {
        if unit.hasPrefix("d") { return fmt(addDays(today, n)) }
        if unit.hasPrefix("w") { return fmt(addDays(today, n * 7)) }
        // months
        return gregorian.date(byAdding: .month, value: n, to: today).map { fmt($0) }
    }

    // "<N>d" / "<N>w" shorthand
    let shortNRe = try! NSRegularExpression(pattern: "^(\\d+)(d|w)$")
    if let m = shortNRe.firstMatch(in: s),
       let nStr = shortNRe.group(1, in: m, source: s),
       let unit = shortNRe.group(2, in: m, source: s),
       let n = Int(nStr) {
        return fmt(addDays(today, unit == "w" ? n * 7 : n))
    }

    return nil
}

// MARK: - Date+Time Input Parsing

func parseDateInput(_ input: String, today: Date = Date()) -> (date: String, time: String?)? {
    let raw = input.trimmingCharacters(in: .whitespaces).lowercased()
    guard !raw.isEmpty else { return nil }

    let (time, rest) = extractTime(raw)
    guard let datePart = parseDatePart(rest, today: today) else { return nil }
    return (datePart, time)
}

// MARK: - Public API

enum DateParser {
    static func parse(_ input: String, today: Date = Date()) -> ParsedDateTimeRecurrence? {
        let raw = input.trimmingCharacters(in: .whitespaces).lowercased()
        guard !raw.isEmpty else { return nil }
        let (field, afterField) = extractField(raw)
        let recExtracted = extractRecurrence(afterField)
        if let parsed = parseDateInput(recExtracted.rest, today: today) {
            return ParsedDateTimeRecurrence(
                date: parsed.date,
                time: parsed.time,
                recurrence: recExtracted.recurrence,
                field: field
            )
        }
        // Bare recurrence (no date) — anchor to today. Must re-check against
        // the UNSTRIPPED afterField (not short-circuit on
        // recExtracted.recurrence) — extractRecurrence can strip a
        // *trailing* recurrence tail off a longer phrase that still has
        // unparseable prose in front (e.g. "call the doctor every sun" →
        // tail "every sun", rest "call the doctor"). That tail match alone
        // doesn't mean the WHOLE input is a bare recurrence phrase; only
        // accept it here when afterField itself is nothing but the
        // recurrence phrase.
        let bareRec = parseRecurrenceInput(afterField)
        if let bareRec = bareRec {
            return ParsedDateTimeRecurrence(date: fmt(today), time: nil, recurrence: bareRec, field: field)
        }
        return nil
    }
}
