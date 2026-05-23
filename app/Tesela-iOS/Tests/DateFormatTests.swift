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
