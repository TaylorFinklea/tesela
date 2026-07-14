import XCTest

final class RelocationWiringTests: XCTestCase {
    private var projectRoot: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    private func source(_ relativePath: String) throws -> String {
        try String(
            contentsOf: projectRoot.appendingPathComponent(relativePath),
            encoding: .utf8
        )
    }

    private func section(
        _ source: String,
        from start: String,
        to end: String
    ) throws -> String {
        let startRange = try XCTUnwrap(source.range(of: start))
        let tail = source[startRange.lowerBound...]
        let endRange = try XCTUnwrap(tail.range(of: end))
        return String(tail[..<endRange.lowerBound])
    }

    func testEveryActionableDailyAndPageMovePresentsTheRealMoveSheet() throws {
        let owners = [
            ("Sources/Views/DailyView.swift", 3),
            ("Sources/Views/PageView.swift", 1),
            ("Sources/Graphite/Views/GrDailyView.swift", 3),
            ("Sources/Graphite/Views/GrPageView.swift", 1),
        ]

        for (relativePath, expectedMoveArms) in owners {
            let owner = try source(relativePath)
            XCTAssertTrue(
                owner.contains(".sheet(item: $moveIntent)"),
                "\(relativePath) must present the move intent"
            )
            XCTAssertTrue(
                owner.contains("BlockMoveSheet(mosaic: mosaic, intent: intent)"),
                "\(relativePath) must present the durable move sheet"
            )
            XCTAssertTrue(
                owner.contains("BlockMoveIntent("),
                "\(relativePath) must construct an exact move intent"
            )

            var moveArms = 0
            var cursor = owner.startIndex
            while let move = owner.range(
                of: "case .moveTo",
                range: cursor..<owner.endIndex
            ) {
                moveArms += 1
                let nextCase = owner.range(
                    of: "case .",
                    range: move.upperBound..<owner.endIndex
                )?.lowerBound ?? owner.endIndex
                let arm = String(owner[move.lowerBound..<nextCase])
                XCTAssertTrue(
                    arm.contains("presentMove(")
                        || arm.contains("moveIntent = BlockMoveIntent("),
                    "\(relativePath) contains an inert Move to action: \(arm)"
                )
                cursor = nextCase
            }
            XCTAssertEqual(moveArms, expectedMoveArms, relativePath)
        }
    }

    func testRelayTickerEnqueueWrappersReserveBeforeCreatingTheirTasks() throws {
        let ticker = try source("Sources/Data/RelayTicker.swift")
        for (start, end) in [
            ("func enqueueRecordAndPush(", "private func recordAndPushUnderLease"),
            ("func enqueueSpliceAndPush(", "private func spliceAndPushUnderLease"),
        ] {
            let wrapper = try section(ticker, from: start, to: end)
            XCTAssertTrue(wrapper.contains("enqueueEngineOperation("))
        }

        let enqueue = try section(
            ticker,
            from: "private func enqueueEngineOperation(",
            to: "private func isCurrentEngineSession"
        )
        let reservation = try XCTUnwrap(enqueue.range(
            of: "engineOperationAdmission.reserveOperation()"
        ))
        let task = try XCTUnwrap(enqueue.range(of: "Task {"))
        XCTAssertLessThan(
            enqueue.distance(from: enqueue.startIndex, to: reservation.lowerBound),
            enqueue.distance(from: enqueue.startIndex, to: task.lowerBound),
            "engine admission must reserve or queue synchronously before a task is scheduled"
        )

        let legacyShell = try source("Sources/Views/AppShell.swift")
        let graphiteShell = try source("Sources/Graphite/Shell/GrAppShell.swift")
        XCTAssertTrue(legacyShell.contains("enqueueRecordAndPush("))
        XCTAssertTrue(graphiteShell.contains("enqueueRecordAndPush("))
        XCTAssertTrue(graphiteShell.contains("enqueueSpliceAndPush("))
    }

    func testWholeNoteSchedulersReserveTheBackendBeforeCreatingTheirTasks() throws {
        let service = try source("Sources/Data/MockMosaicService.swift")
        let page = try section(
            service,
            from: "func schedulePagePush(",
            to: "private func extractFrontmatter"
        )
        let daily = try section(
            service,
            from: "private func scheduleWriteback()",
            to: "// MARK: - HTTP plumbing"
        )
        XCTAssertTrue(page.contains("enqueueBackendMutation"))
        XCTAssertTrue(daily.contains("enqueueBackendMutation"))
    }
}
