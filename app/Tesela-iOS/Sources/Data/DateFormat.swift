import Foundation

enum DateFormat {
    /// Human-readable rendering of a date property value. Accepts bare
    /// `YYYY-MM-DD` (optionally ` HH:mm`) or legacy `[[YYYY-MM-DD]]`.
    /// Unrecognized input returned trimmed-unchanged.
    /// Mirrors `web/src/lib/date-format.ts::formatDateMonthDay`.
    static func humanMonthDay(_ v: String) -> String {
        let trimmed = v.trimmingCharacters(in: .whitespaces)
        let patterns = [
            #"^\[\[(\d{4})-(\d{2})-(\d{2})\]\](?:\s+(\d{2}):(\d{2}))?$"#,
            #"^(\d{4})-(\d{2})-(\d{2})(?:\s+(\d{2}):(\d{2}))?$"#,
        ]
        for p in patterns {
            let re = try! NSRegularExpression(pattern: p)
            guard let m = re.firstMatch(in: trimmed, range: NSRange(trimmed.startIndex..., in: trimmed)) else { continue }
            guard let yr = Range(m.range(at: 1), in: trimmed),
                  let mr = Range(m.range(at: 2), in: trimmed),
                  let dr = Range(m.range(at: 3), in: trimmed) else { continue }
            let y = Int(trimmed[yr])!, mo = Int(trimmed[mr])!, d = Int(trimmed[dr])!
            var dc = DateComponents(); dc.year = y; dc.month = mo; dc.day = d
            guard let date = Calendar(identifier: .gregorian).date(from: dc) else { return trimmed }
            let monthFmt = DateFormatter()
            monthFmt.locale = Locale(identifier: "en_US_POSIX")
            monthFmt.dateFormat = "MMM"
            let monthLabel = monthFmt.string(from: date)
            let thisYear = Calendar.current.component(.year, from: Date())
            let dateStr = y == thisYear ? "\(monthLabel) \(d)" : "\(monthLabel) \(d), \(y)"

            // Optional time — only present when both HH and MM matched (groups 4 and 5).
            if m.numberOfRanges >= 6,
               m.range(at: 4).location != NSNotFound,
               let hr = Range(m.range(at: 4), in: trimmed),
               let mr2 = Range(m.range(at: 5), in: trimmed) {
                let hh = Int(trimmed[hr])!, mm = Int(trimmed[mr2])!
                let ampm = hh >= 12 ? "p" : "a"
                var h = hh % 12
                if h == 0 { h = 12 }
                let minStr = mm == 0 ? "" : String(format: ":%02d", mm)
                return "\(dateStr) \(h)\(minStr)\(ampm)"
            }
            return dateStr
        }
        return trimmed
    }
}
