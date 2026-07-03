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
        XCTAssertEqual(DateParser.parse("today",     today: fixed(2026, 5, 22))?.date, "2026-05-22")
        XCTAssertEqual(DateParser.parse("tomorrow",  today: fixed(2026, 5, 22))?.date, "2026-05-23")
        XCTAssertEqual(DateParser.parse("yesterday", today: fixed(2026, 5, 22))?.date, "2026-05-21")
    }

    func testNextWeekday() {
        XCTAssertEqual(DateParser.parse("next fri", today: fixed(2026, 5, 22))?.date, "2026-05-29")
        XCTAssertEqual(DateParser.parse("fri",      today: fixed(2026, 5, 22))?.date, "2026-05-22")
        XCTAssertEqual(DateParser.parse("mon",      today: fixed(2026, 5, 22))?.date, "2026-05-25")
    }

    func testInNDays() {
        XCTAssertEqual(DateParser.parse("in 3 days", today: fixed(2026, 5, 22))?.date, "2026-05-25")
        XCTAssertEqual(DateParser.parse("3d",        today: fixed(2026, 5, 22))?.date, "2026-05-25")
        XCTAssertEqual(DateParser.parse("2w",        today: fixed(2026, 5, 22))?.date, "2026-06-05")
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
        XCTAssertEqual(DateParser.parse("fri weekly",              today: fixed(2026, 5, 22))?.recurrence, "weekly")
        XCTAssertEqual(DateParser.parse("may 1 every 2 weeks",     today: fixed(2026, 5, 22))?.recurrence, "every 2 weeks")
        XCTAssertEqual(DateParser.parse("fri every mon, wed, fri", today: fixed(2026, 5, 22))?.recurrence, "every mon, wed, fri")
    }

    func testEndClauses() {
        XCTAssertEqual(DateParser.parse("fri weekly until 2026-12-31", today: fixed(2026, 5, 22))?.recurrence, "weekly until 2026-12-31")
        XCTAssertEqual(DateParser.parse("fri weekly count 10",          today: fixed(2026, 5, 22))?.recurrence, "weekly count 10")
        XCTAssertNil(DateParser.parse("fri weekly until 2026-02-30",   today: fixed(2026, 5, 22))?.recurrence)
    }

    func testFieldKeyword() {
        let fri = fixed(2026, 5, 22)
        XCTAssertEqual(DateParser.parse("deadline friday",    today: fri)?.field, .deadline)
        XCTAssertEqual(DateParser.parse("scheduled tomorrow", today: fri)?.field, .scheduled)
        XCTAssertEqual(DateParser.parse("due may 1",          today: fri)?.field, .deadline)
        XCTAssertNil(DateParser.parse("tomorrow",             today: fri)?.field)
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
        XCTAssertEqual(DateParser.parse("weekdays",    today: fri)?.recurrence, "weekdays")
        XCTAssertEqual(DateParser.parse("every 3 days", today: fri)?.recurrence, "every 3 days")
    }

    func testTrailingRecurrenceWithUnparseablePrefixDoesNotBecomeBareRecurrence() {
        XCTAssertNil(DateParser.parse("Call the doctor every sun", today: fixed(2026, 5, 22)))
    }

    func testEmptyAndUnrecognized() {
        XCTAssertNil(DateParser.parse("",           today: fixed(2026, 5, 22)))
        XCTAssertNil(DateParser.parse("not a date", today: fixed(2026, 5, 22)))
    }
}
