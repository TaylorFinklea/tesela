import Foundation

/// Human-readable rendering of a `recurring::` property value.
/// Swift port of `web/src/lib/recurrence-format.ts` — behavior is
/// identical: unrecognized input is returned **unchanged** (never crashes).
enum RecurrenceFormat {
    static func human(_ value: String) -> String {
        // Trim, lowercase, collapse internal whitespace.
        let s = value
            .trimmingCharacters(in: .whitespaces)
            .lowercased()
            .replacingOccurrences(of: #"\s+"#, with: " ", options: .regularExpression)
        guard !s.isEmpty else { return value }

        var base = s
        var endText = ""

        // Split a trailing ` until YYYY-MM-DD` or ` count N`.
        if let untilRange = rangeOfLastOccurrence(of: " until ", in: s) {
            base = String(s[s.startIndex ..< untilRange.lowerBound])
            let dateStr = String(s[untilRange.upperBound...]).trimmingCharacters(in: .whitespaces)
            endText = parseUntilDate(dateStr)
        } else if let countRange = rangeOfLastOccurrence(of: " count ", in: s) {
            base = String(s[s.startIndex ..< countRange.lowerBound])
            let n = String(s[countRange.upperBound...]).trimmingCharacters(in: .whitespaces)
            endText = ", \(n)×"
        }

        guard let freq = formatFreq(base) else { return value }
        return freq + endText
    }

    // ── Private helpers ──────────────────────────────────────────────────

    /// Returns the range of the last occurrence of `needle` in `haystack`,
    /// or `nil` if not found.
    private static func rangeOfLastOccurrence(of needle: String, in haystack: String) -> Range<String.Index>? {
        var last: Range<String.Index>? = nil
        var searchFrom = haystack.startIndex
        while let r = haystack.range(of: needle, range: searchFrom ..< haystack.endIndex) {
            last = r
            searchFrom = r.upperBound
        }
        return last
    }

    /// Parse `YYYY-MM-DD` and return ` until MMM d, yyyy` (e.g. " until Dec 31, 2026"),
    /// or `""` if unparseable.
    private static func parseUntilDate(_ dateStr: String) -> String {
        let parser = DateFormatter()
        parser.dateFormat = "yyyy-MM-dd"
        parser.locale = Locale(identifier: "en_US_POSIX")
        parser.timeZone = TimeZone(secondsFromGMT: 0)
        guard let date = parser.date(from: dateStr) else { return "" }

        let formatter = DateFormatter()
        formatter.dateFormat = "MMM d, yyyy"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        return " until \(formatter.string(from: date))"
    }

    private static let dayLabel: [String: String] = [
        "mon": "Mon", "tue": "Tue", "wed": "Wed", "thu": "Thu",
        "fri": "Fri", "sat": "Sat", "sun": "Sun",
    ]

    private static let otherUnitLabel: [String: String] = [
        "day": "day", "days": "day",
        "week": "week", "weeks": "week",
        "month": "month", "months": "month",
        "year": "year", "years": "year",
    ]

    /// Maps a normalized frequency `base` string to a human label,
    /// or returns `nil` for unrecognized input.
    private static func formatFreq(_ base: String) -> String? {
        switch base {
        case "daily":    return "Daily"
        case "weekly":   return "Weekly"
        case "monthly":  return "Monthly"
        case "yearly":   return "Yearly"
        // Single-word cadences (Rust recurrence.rs, added 2026-06-20).
        case "biweekly": return "Biweekly"
        case "fortnightly": return "Fortnightly"
        case "quarterly": return "Quarterly"
        case "weekdays": return "Weekdays"
        case "every weekday", "every weekdays": return "Weekdays"
        case "weekends": return "Weekends"
        default: break
        }

        guard base.hasPrefix("every ") else { return nil }
        let rest = String(base.dropFirst(6)) // drop "every "

        // `every mon, wed, fri` — all comma-tokens must be known day names.
        let tokens = rest.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }
        if !tokens.isEmpty && tokens.allSatisfy({ dayLabel[$0] != nil }) {
            return tokens.compactMap { dayLabel[$0] }.joined(separator: ", ")
        }

        // `every other <unit>` → interval 2 (added 2026-06-20).
        if rest.hasPrefix("other ") {
            let unit = String(rest.dropFirst(6))
            guard let label = otherUnitLabel[unit] else { return nil }
            return "Every other \(label)"
        }

        // `every N days|weeks|months|years`
        let nUnitPattern = #"^(\d+) (days?|weeks?|months?|years?)$"#
        if let regex = try? NSRegularExpression(pattern: nUnitPattern),
           let match = regex.firstMatch(in: rest, range: NSRange(rest.startIndex..., in: rest)),
           match.numberOfRanges == 3,
           let nRange = Range(match.range(at: 1), in: rest),
           let unitRange = Range(match.range(at: 2), in: rest) {
            return "Every \(rest[nRange]) \(rest[unitRange])"
        }

        return nil
    }
}
