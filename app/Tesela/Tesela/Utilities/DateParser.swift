import Foundation

// MARK: - DateParser
// Parses natural language date strings into ISO dates.
// Used by ⌘K palette search and date picker text input.
//
// Supported formats:
//   "March 23", "Mar 23rd", "3/23", "3-23", "3/23/2026"
//   "today", "tomorrow", "yesterday"
//   "+3d" (3 days from now), "+1w" (1 week), "+2m" (2 months)
//   "fri", "friday", "next friday"
//   "2026-03-23" (ISO passthrough)

enum DateParser {
    /// Parse a natural date string into "YYYY-MM-DD" format. Returns nil if unparseable.
    static func parse(_ input: String) -> String? {
        let q = input.trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return nil }

        let outputFmt = DateFormatter()
        outputFmt.dateFormat = "yyyy-MM-dd"

        // ISO passthrough
        if q.matches(of: /^\d{4}-\d{2}-\d{2}$/).count > 0 {
            return q
        }

        // Relative: today, tomorrow, yesterday
        switch q.lowercased() {
        case "today":     return outputFmt.string(from: Date())
        case "tomorrow":  return outputFmt.string(from: Calendar.current.date(byAdding: .day, value: 1, to: Date())!)
        case "yesterday": return outputFmt.string(from: Calendar.current.date(byAdding: .day, value: -1, to: Date())!)
        default: break
        }

        // Relative offset: +3d, +1w, +2m, +1y
        if let offset = parseRelativeOffset(q) {
            return outputFmt.string(from: offset)
        }

        // Day of week: "fri", "friday", "next friday"
        if let weekday = parseDayOfWeek(q) {
            return outputFmt.string(from: weekday)
        }

        // Natural date formats
        let currentYear = Calendar.current.component(.year, from: Date())
        let fmt = DateFormatter()
        fmt.locale = Locale(identifier: "en_US")

        // Strip ordinal suffixes
        let cleaned = q.replacingOccurrences(of: #"(\d+)(st|nd|rd|th)"#, with: "$1", options: .regularExpression)

        let formats = [
            "MMMM d, yyyy",     // March 23, 2026
            "MMMM d yyyy",      // March 23 2026
            "MMMM d",           // March 23
            "MMM d, yyyy",      // Mar 23, 2026
            "MMM d yyyy",       // Mar 23 2026
            "MMM d",            // Mar 23
            "M/d/yyyy",         // 3/23/2026
            "M/d",              // 3/23
            "M-d-yyyy",         // 3-23-2026
            "M-d",              // 3-23
        ]

        for format in formats {
            fmt.dateFormat = format
            if var date = fmt.date(from: cleaned) {
                if !format.contains("y") {
                    var components = Calendar.current.dateComponents([.month, .day], from: date)
                    components.year = currentYear
                    if let adjusted = Calendar.current.date(from: components) {
                        date = adjusted
                    }
                }
                return outputFmt.string(from: date)
            }
        }

        return nil
    }

    /// Preview: returns a formatted display string for the parsed date, or nil
    static func preview(_ input: String) -> String? {
        guard let iso = parse(input) else { return nil }
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-dd"
        guard let date = fmt.date(from: iso) else { return nil }
        let display = DateFormatter()
        display.dateFormat = "EEEE, MMMM d, yyyy"
        return display.string(from: date)
    }

    // +3d, +1w, +2m, +1y
    private static func parseRelativeOffset(_ q: String) -> Date? {
        let pattern = /^\+(\d+)([dwmy])$/
        guard let match = q.lowercased().firstMatch(of: pattern),
              let count = Int(match.output.1) else { return nil }
        let unit = String(match.output.2)
        let component: Calendar.Component = switch unit {
        case "d": .day
        case "w": .weekOfYear
        case "m": .month
        case "y": .year
        default: .day
        }
        return Calendar.current.date(byAdding: component, value: count, to: Date())
    }

    // "fri", "friday", "next friday"
    private static func parseDayOfWeek(_ q: String) -> Date? {
        let lower = q.lowercased().replacingOccurrences(of: "next ", with: "")
        let days: [String: Int] = [
            "sun": 1, "sunday": 1,
            "mon": 2, "monday": 2,
            "tue": 3, "tuesday": 3,
            "wed": 4, "wednesday": 4,
            "thu": 5, "thursday": 5,
            "fri": 6, "friday": 6,
            "sat": 7, "saturday": 7,
        ]
        guard let target = days[lower] else { return nil }
        let today = Calendar.current.component(.weekday, from: Date())
        var daysAhead = target - today
        if daysAhead <= 0 { daysAhead += 7 }
        return Calendar.current.date(byAdding: .day, value: daysAhead, to: Date())
    }
}
