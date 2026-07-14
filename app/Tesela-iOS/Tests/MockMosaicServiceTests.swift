import XCTest
@testable import Tesela

private actor VoiceTranscriptionResponseGate {
    private var isReleased = false
    private var waiters: [CheckedContinuation<Void, Never>] = []

    func wait() async {
        guard !isReleased else { return }
        await withCheckedContinuation { continuation in
            waiters.append(continuation)
        }
    }

    func release() {
        isReleased = true
        let pending = waiters
        waiters.removeAll()
        for waiter in pending { waiter.resume() }
    }
}

private actor BackendMutationStartGate {
    private var isReleased = false
    private var waiters: [CheckedContinuation<Void, Never>] = []

    func wait() async {
        guard !isReleased else { return }
        await withCheckedContinuation { continuation in
            waiters.append(continuation)
        }
    }

    func release() {
        isReleased = true
        let pending = waiters
        waiters.removeAll()
        for waiter in pending { waiter.resume() }
    }
}

private final class BackendMutationRequestRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var hosts: [String] = []

    func append(host: String) {
        lock.lock()
        hosts.append(host)
        lock.unlock()
    }

    func snapshot() -> [String] {
        lock.lock()
        defer { lock.unlock() }
        return hosts
    }
}

private final class BackendMutationURLProtocol: URLProtocol {
    static var recorder = BackendMutationRequestRecorder()
    static var requestReceived: XCTestExpectation?

    override class func canInit(with request: URLRequest) -> Bool {
        request.url?.host == "profile-a.test" || request.url?.host == "profile-b.test"
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let url = request.url else { return }
        Self.recorder.append(host: url.host ?? "")
        Self.requestReceived?.fulfill()
        let response = HTTPURLResponse(
            url: url,
            statusCode: 200,
            httpVersion: "HTTP/1.1",
            headerFields: nil
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data())
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

private final class DelayedVoiceTranscriptionURLProtocol: URLProtocol {
    static var requestStarted: XCTestExpectation?
    static var responseGate = VoiceTranscriptionResponseGate()
    private var responseTask: Task<Void, Never>?

    override class func canInit(with request: URLRequest) -> Bool {
        request.url?.host == "voice-race.test"
            && request.url?.path == "/transcription/transcribe"
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        Self.requestStarted?.fulfill()
        responseTask = Task { [weak self] in
            await Self.responseGate.wait()
            guard let self, !Task.isCancelled, let url = request.url else { return }
            let data = Data(#"{"text":"profile A transcript","model_id":"test","duration_ms":1}"#.utf8)
            let response = HTTPURLResponse(
                url: url,
                statusCode: 200,
                httpVersion: "HTTP/1.1",
                headerFields: ["Content-Type": "application/json"]
            )!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        }
    }

    override func stopLoading() {
        responseTask?.cancel()
        responseTask = nil
    }
}

/// Tests for MockMosaicService internals that are testable without a live
/// server — specifically the `parseBlocks(from:noteId:)` line-number
/// tracking introduced for the `recur-bump` block_id fix.
@MainActor
final class MockMosaicServiceTests: XCTestCase {

    func testEnqueuedMutationReservesProfileBeforeTaskRuns() async throws {
        let requestReceived = expectation(description: "profile A mutation sent")
        BackendMutationURLProtocol.recorder = BackendMutationRequestRecorder()
        BackendMutationURLProtocol.requestReceived = requestReceived
        defer { BackendMutationURLProtocol.requestReceived = nil }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [BackendMutationURLProtocol.self]
        let service = MockMosaicService(
            session: URLSession(configuration: configuration)
        )
        let profileA = MockMosaicService.Backend.http(
            URL(string: "https://profile-a.test")!
        )
        let profileB = MockMosaicService.Backend.http(
            URL(string: "https://profile-b.test")!
        )
        service.attach(backend: profileA)

        let mutationGate = BackendMutationStartGate()
        let mutationTask = service.enqueueBackendMutation { reservation in
            await mutationGate.wait()
            try await service.setBlockProperty(
                blockId: "daily-a:block-a",
                key: "status",
                value: "done",
                reservation: reservation
            )
        }

        XCTAssertEqual(
            service.testableBackendOperationsInFlight(),
            1,
            "the A intent must reserve the activation barrier synchronously"
        )

        var activationFinished = false
        let activationTask = Task { @MainActor in
            await service.waitUntilBackendOperationsFinish()
            service.attach(backend: profileB)
            activationFinished = true
        }
        await Task.yield()
        XCTAssertFalse(activationFinished)

        await mutationGate.release()
        try await mutationTask.value
        await fulfillment(of: [requestReceived], timeout: 2)
        await activationTask.value

        XCTAssertTrue(activationFinished)
        XCTAssertEqual(
            BackendMutationURLProtocol.recorder.snapshot(),
            ["profile-a.test"],
            "the reserved A mutation must never be redirected to profile B"
        )
        XCTAssertEqual(service.testableBackendOperationsInFlight(), 0)
    }

    func testVoiceCaptureBlocksProfileActivationUntilTranscriptIsCaptured() async throws {
        let requestStarted = expectation(description: "transcription request started")
        DelayedVoiceTranscriptionURLProtocol.requestStarted = requestStarted
        DelayedVoiceTranscriptionURLProtocol.responseGate = VoiceTranscriptionResponseGate()
        defer {
            DelayedVoiceTranscriptionURLProtocol.requestStarted = nil
        }

        let audioURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathExtension("wav")
        try Data([0]).write(to: audioURL)
        defer { try? FileManager.default.removeItem(at: audioURL) }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [DelayedVoiceTranscriptionURLProtocol.self]
        let service = MockMosaicService(
            session: URLSession(configuration: configuration)
        )
        let profileA = MockMosaicService.Backend.http(
            URL(string: "https://voice-race.test")!
        )
        let profileB = MockMosaicService.Backend.http(
            URL(string: "https://profile-b.test")!
        )
        service.attach(backend: profileA)

        let captureTask = Task { @MainActor in
            try await service.captureVoiceNote(audio: audioURL)
        }
        await fulfillment(of: [requestStarted], timeout: 2)

        var activationFinished = false
        let activationTask = Task { @MainActor in
            await service.waitUntilBackendOperationsFinish()
            service.attach(backend: profileB)
            activationFinished = true
        }
        try await Task.sleep(nanoseconds: 50_000_000)

        XCTAssertFalse(
            activationFinished,
            "profile activation must drain the complete transcription and capture lifecycle"
        )
        XCTAssertEqual(service.testableBackendOperationsInFlight(), 1)

        await DelayedVoiceTranscriptionURLProtocol.responseGate.release()
        let capturedTranscript = try await captureTask.value
        XCTAssertEqual(capturedTranscript, "profile A transcript")
        await activationTask.value

        XCTAssertTrue(activationFinished)
        XCTAssertEqual(service.testableBackendOperationsInFlight(), 0)
        XCTAssertFalse(
            service.todayBlocks.contains { $0.text == "profile A transcript" },
            "profile A's transcript must be cleared before profile B becomes active"
        )
    }

    func testSameBackendReattachRejectsAQueuedPageWriteFromOldGeneration() async {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        let oldGeneration = service.testableBackendGeneration()
        var writes = 0
        service.onLocalWrite = { _, _, _, _ in writes += 1 }

        service.attach(backend: .relay)
        await service.pushPage(
            id: "queued-old-profile",
            blocks: [Block(id: "b1", kind: .note, text: "must not write")],
            expectedGeneration: oldGeneration
        )

        XCTAssertEqual(writes, 0)
        XCTAssertNotEqual(service.testableBackendGeneration(), oldGeneration)
    }

    func testScheduledPageWriteReservesActivationBeforeItsTaskStarts() async {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        var writes: [String] = []
        service.onLocalWrite = { slug, _, _, _ in writes.append(slug) }

        service.schedulePagePush(
            id: "profile-a-page",
            blocks: [Block(id: "b1", kind: .note, text: "durable A edit")]
        )

        XCTAssertFalse(service.backendActivationIsSafe)
        XCTAssertEqual(service.testableBackendOperationsInFlight(), 1)

        var activationFinished = false
        let activation = Task { @MainActor in
            await service.waitUntilBackendOperationsFinish()
            service.attach(backend: .relay)
            activationFinished = true
        }
        await activation.value

        XCTAssertTrue(activationFinished)
        XCTAssertEqual(writes, ["profile-a-page"])
        XCTAssertTrue(service.backendActivationIsSafe)
    }

    // MARK: - Block folding

    func testBlockFoldVisibleBlocksHideCollapsedDescendantsOnly() {
        let blocks = [
            Block(id: "root", kind: .note, text: "root", indent: 0),
            Block(id: "child-a", kind: .note, text: "child a", indent: 1),
            Block(id: "grandchild", kind: .note, text: "grandchild", indent: 2),
            Block(id: "child-b", kind: .note, text: "child b", indent: 1),
            Block(id: "sibling", kind: .note, text: "sibling", indent: 0),
        ]

        let visible = BlockFold.visibleBlocks(in: blocks, collapsed: ["root"])

        XCTAssertEqual(visible.map(\.id), ["root", "sibling"])
    }

    func testBlockFoldNestedCollapseResumesAtMatchingIndent() {
        let blocks = [
            Block(id: "root", kind: .note, text: "root", indent: 0),
            Block(id: "child-a", kind: .note, text: "child a", indent: 1),
            Block(id: "grandchild", kind: .note, text: "grandchild", indent: 2),
            Block(id: "child-b", kind: .note, text: "child b", indent: 1),
            Block(id: "sibling", kind: .note, text: "sibling", indent: 0),
        ]

        let visible = BlockFold.visibleBlocks(in: blocks, collapsed: ["child-a"])

        XCTAssertEqual(visible.map(\.id), ["root", "child-a", "child-b", "sibling"])
    }

    func testBlockFoldHasChildrenUsesUnderlyingBlockOrder() {
        let blocks = [
            Block(id: "root", kind: .note, text: "root", indent: 0),
            Block(id: "child", kind: .note, text: "child", indent: 1),
            Block(id: "sibling", kind: .note, text: "sibling", indent: 0),
        ]

        XCTAssertTrue(BlockFold.hasChildren(block: blocks[0], in: blocks))
        XCTAssertFalse(BlockFold.hasChildren(block: blocks[1], in: blocks))
        XCTAssertFalse(BlockFold.hasChildren(block: blocks[2], in: blocks))
    }

    // MARK: - Past daily editing

    func testEditPastDailyBlockUpdatesFeedEntryAndLoadedPage() async {
        let service = MockMosaicService()
        service.testableSetPastDailies([
            DailyEntry(
                id: "2026-06-08",
                blocks: [
                    Block(id: "old-root", kind: .note, text: "old root"),
                    Block(id: "old-child", kind: .note, text: "old child", indent: 1),
                ]
            )
        ])

        service.editPastDailyBlock(dayId: "2026-06-08", blockId: "old-child", text: "edited child #done")
        await service.testableWaitForPendingTasks()

        XCTAssertEqual(service.pastDailies[0].blocks[1].text, "edited child")
        XCTAssertEqual(service.pastDailies[0].blocks[1].rawText, "edited child")
        XCTAssertEqual(service.pastDailies[0].blocks[1].tags, ["#done"])
        XCTAssertEqual(service.loadedPageBlocks["2026-06-08"]?[1].text, "edited child")
        XCTAssertEqual(service.loadedPageBlocks["2026-06-08"]?[1].tags, ["#done"])
    }

    func testRelayRefreshUsesCurrentCalendarDayAfterServiceStayedAliveAcrossMidnight() async throws {
        let previousDay = "2099-06-11"
        let currentDay = "2099-06-12"
        try resetLocalDailyFixtures([previousDay, currentDay])
        defer { try? resetLocalDailyFixtures([previousDay, currentDay]) }
        try writeLocalDaily(id: previousDay, body: "- old launch-day block")
        try writeLocalDaily(id: currentDay, body: "- current calendar-day block")

        var now = date(2099, 6, 11)
        let service = MockMosaicService(now: { now })
        service.attach(backend: .relay)

        await service.refresh(from: .relay)
        XCTAssertEqual(service.todayDailySlug, previousDay)
        XCTAssertEqual(service.todayBlocks.map(\.text), ["old launch-day block"])

        now = date(2099, 6, 12)
        await service.refresh(from: .relay)

        XCTAssertEqual(service.todayDailySlug, currentDay)
        XCTAssertEqual(service.todayBlocks.map(\.text), ["current calendar-day block"])
        XCTAssertEqual(service.yesterdayBlocks.map(\.text), ["old launch-day block"])
    }

    func testRelayRefreshClearsStaleTodayBlocksWhenNewDayHasNoLocalDailyYet() async throws {
        let previousDay = "2099-06-13"
        let currentDay = "2099-06-14"
        try resetLocalDailyFixtures([previousDay, currentDay])
        defer { try? resetLocalDailyFixtures([previousDay, currentDay]) }
        try writeLocalDaily(id: previousDay, body: "- previous day block")

        var now = date(2099, 6, 13)
        let service = MockMosaicService(now: { now })
        service.attach(backend: .relay)

        await service.refresh(from: .relay)
        XCTAssertEqual(service.todayBlocks.map(\.text), ["previous day block"])

        now = date(2099, 6, 14)
        await service.refresh(from: .relay)

        XCTAssertEqual(service.todayDailySlug, currentDay)
        XCTAssertTrue(service.todayBlocks.isEmpty)
        XCTAssertEqual(service.yesterdayBlocks.map(\.text), ["previous day block"])
    }

    /// 2026-06-24 device test: a block deleted on web stayed on iOS for ~44min
    /// until a force-close. The engine applies+re-materializes an inbound
    /// BlockDelete (apply_doc_update_status → materialize_note), so today.md
    /// drops the block; the iOS UI must then re-read it when the relay tick's
    /// `onAppliedChanges` seam fires `applyRemoteChange()` — not linger until a
    /// cold start. Guards the refresh link of that chain.
    func testRelayRemoteDeleteIsReflectedAfterApplyRemoteChange() async throws {
        let today = "2099-06-20"
        try resetLocalDailyFixtures([today])
        defer { try? resetLocalDailyFixtures([today]) }
        try writeLocalDaily(id: today, body: "- keep me\n- delete me")

        let now = date(2099, 6, 20)
        let service = MockMosaicService(now: { now })
        service.attach(backend: .relay)
        await service.refresh(from: .relay)
        XCTAssertEqual(service.todayBlocks.map(\.text), ["keep me", "delete me"])

        // Remote BlockDelete arrives: engine re-materialized today.md without it.
        try writeLocalDaily(id: today, body: "- keep me")
        await service.applyRemoteChange()
        try await Task.sleep(nanoseconds: 600_000_000)  // outlast the 300ms debounce

        XCTAssertEqual(
            service.todayBlocks.map(\.text), ["keep me"],
            "a remote delete must drop the block on the next refresh, not linger until cold start")
    }

    /// 2026-06-27 device test: an inbound change to YESTERDAY (a desktop edit /
    /// delete) must re-render on iOS, exactly like TODAY does above. Taylor
    /// reported past days behaving read-only — desktop edits to yesterday never
    /// appeared on iOS. This guards the inbound-refresh link for past days.
    func testRelayRemoteDeleteOnYesterdayIsReflectedAfterApplyRemoteChange() async throws {
        let today = "2099-06-22"
        let yesterday = "2099-06-21"
        try resetLocalDailyFixtures([today, yesterday])
        defer { try? resetLocalDailyFixtures([today, yesterday]) }
        try writeLocalDaily(id: today, body: "- today block")
        try writeLocalDaily(id: yesterday, body: "- keep me\n- delete me")

        let now = date(2099, 6, 22)
        let service = MockMosaicService(now: { now })
        service.attach(backend: .relay)
        await service.refresh(from: .relay)
        XCTAssertEqual(service.yesterdayBlocks.map(\.text), ["keep me", "delete me"])

        // Remote delete on yesterday: engine re-materialized yesterday.md w/o it.
        try writeLocalDaily(id: yesterday, body: "- keep me")
        await service.applyRemoteChange()
        try await Task.sleep(nanoseconds: 600_000_000)  // outlast the 300ms debounce

        XCTAssertEqual(
            service.yesterdayBlocks.map(\.text), ["keep me"],
            "an inbound change to yesterday must re-render on iOS, like today")
    }

    /// Baseline: with NOTHING being edited, an inbound change to an OPEN
    /// non-daily PAGE (GrPageView / `loadedPageBlocks`) must re-render
    /// exactly like today/yesterday do above — `refreshLoadedPages()`
    /// (called from `applyRemoteChange()`'s debounced tail) picks up a
    /// page that's currently loaded.
    func testRelayRemotePageEditIsReflectedAfterApplyRemoteChangeWhilePageIsOpen() async throws {
        let pageId = "diagnostic-page-ect"
        try resetLocalDailyFixtures([pageId])
        defer { try? resetLocalDailyFixtures([pageId]) }
        try writeLocalDaily(id: pageId, body: "- original text")

        let service = MockMosaicService()
        service.attach(backend: .relay)
        await service.loadPage(id: pageId)
        XCTAssertEqual(service.loadedPageBlocks[pageId]?.map(\.text), ["original text"])

        // Remote convergence merge lands: engine re-materializes the page.
        try writeLocalDaily(id: pageId, body: "- merged text")
        await service.applyRemoteChange()
        try await Task.sleep(nanoseconds: 600_000_000)  // outlast the 300ms debounce

        XCTAssertEqual(
            service.loadedPageBlocks[pageId]?.map(\.text), ["merged text"],
            "a relay-arriving change to an OPEN non-daily page must re-render, not linger until manual refresh")
    }

    /// ROOT CAUSE (tesela-ect): `applyRemoteChange()`'s `isEditingBlock`
    /// branch used to return BEFORE calling `refresh`/`refreshLoadedPages`
    /// at all — so editing a block in TODAY's daily silently starved every
    /// OTHER already-open note (a Library page has zero relation to that
    /// edit) of its live refresh until the edit's blur flushed
    /// `pendingRemoteRefresh`. If the user never blurred (kept the field
    /// focused while just watching the OTHER note), it stayed stale until a
    /// manual pull-to-refresh, which calls `mosaic.refresh`/
    /// `refreshLoadedPages` DIRECTLY, bypassing this gate entirely — exactly
    /// the reported symptom. Guards the note-scoped fix: an unrelated open
    /// page must refresh immediately even mid-edit on the daily, while the
    /// daily itself stays protected from a wholesale reassignment (the
    /// original clobber risk the gate exists for).
    ///
    /// Fix-round update: the refresh now runs through
    /// `scheduleEditingRefresh`'s ~300ms coalescing window rather than
    /// synchronously inline, so this asserts after outlasting it (mirrors
    /// every other debounced-refresh test in this file).
    func testApplyRemoteChangeWhileEditingDailyStillRefreshesOtherOpenPage() async throws {
        let today = "2099-06-25"
        let pageId = "diagnostic-page-ect-2"
        try resetLocalDailyFixtures([today, pageId])
        defer { try? resetLocalDailyFixtures([today, pageId]) }
        try writeLocalDaily(id: today, body: "- today original")
        try writeLocalDaily(id: pageId, body: "- page original")

        let now = date(2099, 6, 25)
        let service = MockMosaicService(now: { now })
        service.attach(backend: .relay)
        await service.refresh(from: .relay)
        await service.loadPage(id: pageId)
        XCTAssertEqual(service.todayBlocks.map(\.text), ["today original"])
        XCTAssertEqual(service.loadedPageBlocks[pageId]?.map(\.text), ["page original"])

        // User is mid-edit on a TODAY block (mirrors GrDailyView's
        // onChange(of: editingBlockId): isEditingBlock + editingBlockId set,
        // editingScope explicitly `.daily` — the daily family, not a page).
        service.isEditingBlock = true
        service.editingBlockId = service.todayBlocks[0].id
        service.editingScope = .daily

        // A remote merge lands for the UNRELATED page while the daily edit
        // is still focused (no blur).
        try writeLocalDaily(id: pageId, body: "- page merged")
        await service.applyRemoteChange()
        try await Task.sleep(nanoseconds: 600_000_000)  // outlast the 300ms debounce

        XCTAssertEqual(
            service.loadedPageBlocks[pageId]?.map(\.text), ["page merged"],
            "an unrelated open page must refresh immediately, not wait for the daily edit's blur")
        XCTAssertEqual(
            service.todayBlocks.map(\.text), ["today original"],
            "the note actually being edited must stay protected from a wholesale reassignment mid-edit")
        XCTAssertTrue(
            service.isEditingBlock,
            "the refresh above must have landed WITHOUT the daily edit blurring")
    }

    /// Symmetric case: editing a block in a PAGE must not starve TODAY or a
    /// DIFFERENT page of their live refresh, while the edited page itself
    /// keeps the same protection today's daily gets. Doubles as the (c)
    /// anti-regression from the fix-round: other notes must refresh
    /// WITHOUT the page edit ever blurring (`isEditingBlock` stays `true`
    /// throughout — see the final assertion) — a coalesce delay is fine,
    /// blur-gating is not (blur-gating was the ORIGINAL ect bug this whole
    /// note-scoping fix exists to undo).
    func testApplyRemoteChangeWhileEditingPageStillRefreshesTodayAndOtherPage() async throws {
        let today = "2099-06-26"
        let editedPageId = "diagnostic-page-ect-edited"
        let otherPageId = "diagnostic-page-ect-other"
        try resetLocalDailyFixtures([today, editedPageId, otherPageId])
        defer { try? resetLocalDailyFixtures([today, editedPageId, otherPageId]) }
        try writeLocalDaily(id: today, body: "- today original")
        try writeLocalDaily(id: editedPageId, body: "- edited-page original")
        try writeLocalDaily(id: otherPageId, body: "- other-page original")

        let now = date(2099, 6, 26)
        let service = MockMosaicService(now: { now })
        service.attach(backend: .relay)
        await service.refresh(from: .relay)
        await service.loadPage(id: editedPageId)
        await service.loadPage(id: otherPageId)

        // User is mid-edit on a block in `editedPageId` (mirrors
        // GrPageView's onChange(of: editingBlockId)).
        service.isEditingBlock = true
        service.editingBlockId = service.loadedPageBlocks[editedPageId]?.first?.id
        service.editingScope = .page(editedPageId)

        // Remote merges land for TODAY and the OTHER page while that edit
        // is still focused (no blur).
        try writeLocalDaily(id: today, body: "- today merged")
        try writeLocalDaily(id: otherPageId, body: "- other-page merged")
        await service.applyRemoteChange()
        try await Task.sleep(nanoseconds: 600_000_000)  // outlast the 300ms debounce

        XCTAssertEqual(
            service.todayBlocks.map(\.text), ["today merged"],
            "today's daily is unrelated to a page edit and must refresh immediately")
        XCTAssertEqual(
            service.loadedPageBlocks[otherPageId]?.map(\.text), ["other-page merged"],
            "a different open page is unrelated to this page's edit and must refresh immediately")
        XCTAssertEqual(
            service.loadedPageBlocks[editedPageId]?.map(\.text), ["edited-page original"],
            "the page actually being edited must stay protected from a wholesale reassignment mid-edit")
        XCTAssertTrue(
            service.isEditingBlock,
            "today/other-page must have refreshed WITHOUT the page edit ever blurring (anti-regression " +
            "for the original ect bug, which required blur to refresh anything)")
    }

    /// ROOT CAUSE (tesela-ect), second half: `reconcileOpenBlockLive` used
    /// to be hardcoded to `serverDailyId`, so a remote splice landing on a
    /// PAGE's focused block silently no-oped (the engine read was issued
    /// against the wrong note) — it never lived-reconciled, only the
    /// deferred blur refresh could ever show it. Guards that the live
    /// reconcile now routes through `editingScope` and writes the merged
    /// text into `loadedPageBlocks[slug]`, not `todayBlocks`.
    func testApplyRemoteChangeLiveReconcilesFocusedPageBlockIntoLoadedPageBlocks() async throws {
        let pageId = "diagnostic-page-ect-reconcile"
        try resetLocalDailyFixtures([pageId])
        defer { try? resetLocalDailyFixtures([pageId]) }
        try writeLocalDaily(id: pageId, body: "- local text")

        let service = MockMosaicService()
        service.attach(backend: .relay)
        await service.loadPage(id: pageId)
        let bid = try XCTUnwrap(service.loadedPageBlocks[pageId]?.first?.id)

        // `openBlockInserter` is `weak` (BlockRow's `@State` owns the strong
        // reference in production) — hold it here or ARC deallocates it
        // immediately and `reconcileOpenBlockLive` silently no-ops.
        let inserter = CollabTextInserter()
        service.isEditingBlock = true
        service.editingBlockId = bid
        service.editingScope = .page(pageId)
        service.openBlockInserter = inserter
        service.readEngineBlockText = { slug, blockId in
            (slug == pageId && blockId == bid) ? "merged block text" : nil
        }

        await service.applyRemoteChange()
        try await Task.sleep(nanoseconds: 300_000_000)  // let the detached reconcile Task run

        XCTAssertEqual(
            service.loadedPageBlocks[pageId]?.first?.text, "merged block text",
            "a remote splice on a focused PAGE block must live-reconcile into loadedPageBlocks, " +
            "not silently no-op against the wrong (daily) slug")
    }

    /// FIX-ROUND FINDING 1: a burst of N inbound `applyRemoteChange()`
    /// calls while editing must collapse to exactly ONE coalesced refresh
    /// cycle. Un-coalesced, each of the three inbound seams (2s relay
    /// poll, `liveSync.onNoteChange`, per-WS-frame `onBinaryDelta`) drove a
    /// full `refresh(from:)` + `refreshLoadedPages(force: true)` PER EVENT
    /// — stacking concurrent network/local-read calls and reintroducing
    /// the exact main-actor freeze `scheduleRemoteRefresh`'s debounce (T4)
    /// was built to prevent. Counts ACTUAL fires via
    /// `testableEditingRefreshFireCount()` rather than inferring it from
    /// final state — final state alone can't distinguish "collapsed to 1"
    /// from "N ran redundantly but converged to the same result".
    func testApplyRemoteChangeEditingBurstCoalescesToOneRefreshCycle() async throws {
        let today = "2099-06-28"
        let pageId = "diagnostic-page-ect-burst"
        try resetLocalDailyFixtures([today, pageId])
        defer { try? resetLocalDailyFixtures([today, pageId]) }
        try writeLocalDaily(id: today, body: "- today original")
        try writeLocalDaily(id: pageId, body: "- page original")

        let now = date(2099, 6, 28)
        let service = MockMosaicService(now: { now })
        service.attach(backend: .relay)
        await service.refresh(from: .relay)
        await service.loadPage(id: pageId)

        service.isEditingBlock = true
        service.editingBlockId = service.todayBlocks[0].id
        service.editingScope = .daily

        // A burst of N inbound events lands in a tight loop, mirroring
        // several un-batched seams firing in quick succession.
        for _ in 0..<10 {
            await service.applyRemoteChange()
        }
        try await Task.sleep(nanoseconds: 600_000_000)  // outlast the 300ms debounce

        XCTAssertEqual(
            service.testableEditingRefreshFireCount(), 1,
            "a burst of N inbound events while editing must collapse to exactly one refresh cycle, not N")
        XCTAssertEqual(
            service.loadedPageBlocks[pageId]?.map(\.text), ["page original"],
            "sanity: the unrelated open page is still tracked through the coalesced pass")
    }

    /// FIX-ROUND FINDING 2: a legacy caller (`DailyView`/`PageView`, still
    /// shipped behind the `-legacy` flag / `tesela.useLegacyShell`) sets
    /// `isEditingBlock` but never migrated to set `editingScope` — it
    /// stays at the `.unknown` default. That MUST take the conservative
    /// pre-tesela-ect path (defer EVERYTHING to blur), not silently
    /// misclassify it as a daily edit (the old `?? serverDailyId`
    /// fallback) and force-reload an open page out from under the legacy
    /// edit — the exact clobber this gate exists to stop.
    func testApplyRemoteChangeWithUnknownEditingScopeDefersEverythingToBlur() async throws {
        let pageId = "diagnostic-page-ect-legacy"
        try resetLocalDailyFixtures([pageId])
        defer { try? resetLocalDailyFixtures([pageId]) }
        try writeLocalDaily(id: pageId, body: "- page original")

        let service = MockMosaicService()
        service.attach(backend: .relay)
        await service.loadPage(id: pageId)
        XCTAssertEqual(service.loadedPageBlocks[pageId]?.map(\.text), ["page original"])

        // Legacy PageView/DailyView shape: isEditingBlock is set, but
        // editingScope is left at its .unknown default (never migrated).
        service.isEditingBlock = true
        XCTAssertEqual(service.editingScope, .unknown)

        // A remote merge lands for the "open" page while isEditingBlock is
        // true and the scope is unknown.
        try writeLocalDaily(id: pageId, body: "- page merged")
        await service.applyRemoteChange()
        try await Task.sleep(nanoseconds: 600_000_000)  // outlast the debounce window, if any fired

        XCTAssertEqual(
            service.loadedPageBlocks[pageId]?.map(\.text), ["page original"],
            "an UNKNOWN editing scope must defer EVERYTHING to blur, not force-reload a page it " +
            "cannot prove is unrelated to the in-flight (unidentified) edit")
        XCTAssertEqual(
            service.testableEditingRefreshFireCount(), 0,
            "the coalesced editing-refresh pass must never fire for an unknown editing scope")

        // Blurring flushes the deferred refresh, exactly like the
        // pre-tesela-ect behavior.
        service.isEditingBlock = false
        try await Task.sleep(nanoseconds: 600_000_000)
        XCTAssertEqual(
            service.loadedPageBlocks[pageId]?.map(\.text), ["page merged"],
            "blur must flush the deferred refresh, same as before this whole note-scoping fix existed")
    }

    /// 2026-07-02 device flicker fix (tesela-z1e): rapid deletes on
    /// yesterday's blocks re-showed a just-deleted block for a tick. Root
    /// cause: `scheduleWriteback` (today) opens the remote-refresh
    /// suppression window SYNCHRONOUSLY before spawning its writeback
    /// `Task`, but `scheduleYesterdayWriteback` only opened it inside
    /// `pushPage`, which runs INSIDE that async Task — leaving a window
    /// where a same-tick `applyRemoteChange()` (relay tick's
    /// `onAppliedChanges` seam) could see suppression as inactive and
    /// refresh from the not-yet-updated local sandbox file, resurrecting
    /// the deleted block. Guards the fix: deleting a yesterday block must
    /// open suppression before the writeback Task ever runs, mirroring
    /// today's create/delete symmetry.
    func testDeleteYesterdayBlockOpensRemoteSuppressionSynchronously() async throws {
        let today = "2099-06-23"
        let yesterday = "2099-06-22"
        try resetLocalDailyFixtures([today, yesterday])
        defer { try? resetLocalDailyFixtures([today, yesterday]) }
        try writeLocalDaily(id: today, body: "- today block")
        try writeLocalDaily(id: yesterday, body: "- keep me\n- delete me")

        let now = date(2099, 6, 23)
        let service = MockMosaicService(now: { now })
        service.attach(backend: .relay)
        await service.refresh(from: .relay)
        XCTAssertEqual(service.yesterdayBlocks.map(\.text), ["keep me", "delete me"])
        XCTAssertFalse(service.testableIsRemoteSuppressionActive())

        let deleteId = service.yesterdayBlocks[1].id
        service.deleteYesterdayBlock(id: deleteId)

        XCTAssertEqual(service.yesterdayBlocks.map(\.text), ["keep me"])
        XCTAssertTrue(
            service.testableIsRemoteSuppressionActive(),
            "deleting a yesterday block must open remote-refresh suppression synchronously, " +
            "before the async writeback Task runs, so a same-tick applyRemoteChange() can't " +
            "resurrect the just-deleted block from a stale local read")
    }

    /// HONEST CONNECTION STATUS (2026-06-21 silent-desync fix): an
    /// unreachable HTTP backend must flip `connection` to `.failed` even
    /// when local data is on screen — it must NOT sit silently green
    /// `.ready` (the trap that desynced a real device). Reads stay intact
    /// (the local copy is still rendered); only the status becomes
    /// truthful. Guards against a future refactor re-forcing `.ready`.
    func testHttpRefreshSurfacesUnreachableBackendWhileKeepingLocalReads() async throws {
        let today = "2099-06-15"
        try resetLocalDailyFixtures([today])
        defer { try? resetLocalDailyFixtures([today]) }
        try writeLocalDaily(id: today, body: "- local-only block")

        let now = date(2099, 6, 15)
        let service = MockMosaicService(now: { now })

        // Port 1 refuses instantly — a stand-in for a wrong LAN IP / Mac
        // off / 127.0.0.1-on-a-real-device (HTTP fails, local copy exists).
        let dead = MockMosaicService.Backend.http(URL(string: "http://127.0.0.1:1")!)
        service.attach(backend: dead)
        await service.refresh(from: dead)

        // Reads survive — the local copy is still on screen.
        XCTAssertEqual(service.todayBlocks.map(\.text), ["local-only block"])
        // Status is honest: unreachable backend → .failed, NOT green .ready.
        guard case .failed(let message) = service.connection else {
            return XCTFail("expected .failed for an unreachable HTTP backend, got \(service.connection)")
        }
        // ...and it's the calm degraded copy (reads OK), not a raw transport error.
        XCTAssertTrue(
            message.contains("showing your local copy"),
            "expected the degraded message, got: \(message)"
        )
        // The message must NOT overclaim that writes are stuck — edits ride
        // the relay path independently of this HTTP backend.
        XCTAssertFalse(
            message.lowercased().contains("won't sync"),
            "degraded message must not claim writes won't sync: \(message)"
        )
    }

    // MARK: - recurring-task auto-roll in relay mode (2026-06-21)

    /// Completing a recurring task in `.relay` (offline) has no server
    /// route to roll it, so the client rolls locally via the typed-property
    /// engine seam (`onLocalPropertySet`), mirroring the server's
    /// rewrite_block_for_complete: status→todo, dates advanced from their
    /// own value, last_completed stamped, recurrence_done bumped.
    func testRecurringTaskRollsForwardOnCompletion() async {
        let service = MockMosaicService()
        var writes: [(key: String, value: String)] = []
        service.onLocalPropertySet = { _, _, key, value in writes.append((key, value)); return true }
        let props = [
            BlockProperty(key: "status", value: "done"),
            BlockProperty(key: "recurring", value: "weekly"),
            BlockProperty(key: "scheduled", value: "2026-06-15"),
        ]
        let rolled = await service.rollRecurringComplete(noteId: "2026-06-15", bid: "b1", properties: props)
        XCTAssertTrue(rolled)
        let last = Dictionary(writes.map { ($0.key, $0.value) }, uniquingKeysWith: { _, l in l })
        XCTAssertEqual(last["status"], "todo")
        XCTAssertEqual(last["scheduled"], "[[2026-06-22]]")
        XCTAssertEqual(last["last_completed"], "[[2026-06-15]]")
        XCTAssertEqual(last["recurrence_done"], "1")
    }

    /// A dated field's `HH:MM` survives the roll (parity with the server).
    func testRecurringRollPreservesTimeOfDay() async {
        let service = MockMosaicService()
        var writes: [(key: String, value: String)] = []
        service.onLocalPropertySet = { _, _, key, value in writes.append((key, value)); return true }
        let props = [
            BlockProperty(key: "recurring", value: "every 2 days"),
            BlockProperty(key: "deadline", value: "2026-06-15 09:30"),
        ]
        _ = await service.rollRecurringComplete(noteId: "n", bid: "b", properties: props)
        let last = Dictionary(writes.map { ($0.key, $0.value) }, uniquingKeysWith: { _, l in l })
        XCTAssertEqual(last["deadline"], "[[2026-06-17]] 09:30")
    }

    /// A spent series (Count reached) leaves status done and only records
    /// the final completion — no status reset, no date advance.
    func testRecurringSpentSeriesDoesNotRoll() async {
        let service = MockMosaicService()
        var writes: [(key: String, value: String)] = []
        service.onLocalPropertySet = { _, _, key, value in writes.append((key, value)); return true }
        let props = [
            BlockProperty(key: "recurring", value: "weekly count 1"),
            BlockProperty(key: "scheduled", value: "2026-06-15"),
        ]
        let rolled = await service.rollRecurringComplete(noteId: "n", bid: "b", properties: props)
        XCTAssertTrue(rolled)
        // The completion IS persisted (status done) so it doesn't revert on
        // refresh — but the series does NOT advance (dates unchanged).
        XCTAssertEqual(writes.first { $0.key == "status" }?.value, "done")
        XCTAssertFalse(writes.contains { $0.key == "scheduled" })
        XCTAssertEqual(writes.first { $0.key == "recurrence_done" }?.value, "1")
    }

    /// A non-recurring task (or a recurring one with no anchor date) returns
    /// false + writes nothing, so the caller does the plain status:: write.
    func testNonRecurringTaskIsNotRolled() async {
        let service = MockMosaicService()
        var writes: [(key: String, value: String)] = []
        service.onLocalPropertySet = { _, _, key, value in writes.append((key, value)); return true }
        let plain = [BlockProperty(key: "status", value: "done"), BlockProperty(key: "scheduled", value: "2026-06-15")]
        let rolledPlain = await service.rollRecurringComplete(noteId: "n", bid: "b", properties: plain)
        XCTAssertFalse(rolledPlain)
        let anchorless = [BlockProperty(key: "recurring", value: "weekly")]
        let rolledAnchorless = await service.rollRecurringComplete(noteId: "n", bid: "b", properties: anchorless)
        XCTAssertFalse(rolledAnchorless)
        XCTAssertTrue(writes.isEmpty)
    }

    // MARK: - parseBlocks line-number tracking

    /// Verify that each Block's `lineNumber` records the 0-based index of
    /// the `- ` bullet line in the body string. This is the line number
    /// the server's `POST /blocks/recur-bump` route expects as the second
    /// component of `<noteId>:<line>`.
    func testParseBlocksLineNumbers() throws {
        // Build a three-block body. The blocks are at lines 0, 2, and 4;
        // lines 1 and 3 are property sub-lines.
        let body = """
        - First block
          status:: todo
        - Second block
          tags:: Task
        - Third block
        """

        let service = MockMosaicService()
        // Call through the internal helper via a fresh refresh so we can
        // inspect todayBlocks — or reach it directly via the internal
        // `@testable` access.
        let blocks = service.testableParseBlocks(from: body, noteId: "test-note")

        XCTAssertEqual(blocks.count, 3, "Expected 3 blocks")

        XCTAssertEqual(blocks[0].lineNumber, 0, "First block should be at line 0")
        XCTAssertEqual(blocks[0].noteId, "test-note")
        XCTAssertEqual(blocks[0].text, "First block")

        XCTAssertEqual(blocks[1].lineNumber, 2, "Second block should be at line 2")
        XCTAssertEqual(blocks[1].noteId, "test-note")
        XCTAssertEqual(blocks[1].text, "Second block")

        XCTAssertEqual(blocks[2].lineNumber, 4, "Third block should be at line 4")
        XCTAssertEqual(blocks[2].noteId, "test-note")
        XCTAssertEqual(blocks[2].text, "Third block")
    }

    /// Blocks that start at line 0 with no sub-lines should have lineNumber 0.
    func testParseBlocksSingleBlock() {
        let body = "- Only block"
        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "note-abc")
        XCTAssertEqual(blocks.count, 1)
        XCTAssertEqual(blocks[0].lineNumber, 0)
        XCTAssertEqual(blocks[0].noteId, "note-abc")
    }

    /// Empty body should yield no blocks.
    func testParseBlocksEmptyBody() {
        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: "", noteId: "empty-note")
        XCTAssertTrue(blocks.isEmpty)
    }

    /// A block whose body spans multiple lines (continuation lines under
    /// the bullet) should keep every line in `rawText` so the iOS
    /// outliner renders all of them, matching the web client. `text`
    /// remains the first line only for previews/grep.
    ///
    /// Regression test: previously, continuation lines were silently
    /// dropped during parse, which both truncated display on iOS and
    /// caused permanent data loss on writeback (the dropped lines never
    /// made it back to disk after any subsequent edit).
    func testParseBlocksKeepsContinuationLinesInRawText() {
        let body = """
        - best for stable preferences and environment facts
          A realistic "value loop" for you
          1. You ask Hermes to do some annoying multi-step task.
          2. Hermes completes it.
        - next block
        """

        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "multi")

        XCTAssertEqual(blocks.count, 2)
        XCTAssertEqual(blocks[0].text, "best for stable preferences and environment facts")
        XCTAssertEqual(
            blocks[0].rawText,
            """
            best for stable preferences and environment facts
            A realistic "value loop" for you
            1. You ask Hermes to do some annoying multi-step task.
            2. Hermes completes it.
            """
        )
        XCTAssertEqual(blocks[1].text, "next block")
        XCTAssertEqual(blocks[1].rawText, "next block")
    }

    /// Round-trip a multi-line block through parse → render and verify
    /// every continuation line survives. The pre-fix writeback emitted
    /// only `- block.text`, so a multi-line block would collapse to one
    /// line on the first edit anywhere on the daily.
    ///
    /// We include canonical-UUID bid comments so the renderer's
    /// "emit bid suffix" branch produces deterministic output equal
    /// to the input.
    func testRenderBodyPreservesContinuationLines() {
        let body = """
        - first block <!-- bid:4BF3B0E3-BF14-4514-B47A-E8F763066756 -->
          continuation alpha
          continuation beta
        - second block <!-- bid:F4864AC3-2CF0-407B-8895-34548623E794 -->
        """

        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "round")
        let rendered = service.testableRenderBody(from: blocks)

        XCTAssertEqual(rendered, body)
    }

    func testCanonicalLiftedHeadingProseAndFenceDisplayAndEditRoundTrip() {
        let headingBid = "11111111-1111-1111-1111-111111111111"
        let proseBid = "22222222-2222-2222-2222-222222222222"
        let fenceBid = "33333333-3333-3333-3333-333333333333"
        let literalBid = "44444444-4444-4444-4444-444444444444"
        let body = [
            "- # Heading <!-- bid:\(headingBid) -->",
            "- Prose one <!-- bid:\(proseBid) -->",
            "  prose two",
            "- <!-- bid:\(fenceBid) -->",
            "  ```query",
            "  status:: done",
            "  #literal",
            "  - payload, not a block",
            "  ",
            "    extra-indented payload  ",
            "  <!-- bid:\(literalBid) -->",
            "  ```",
        ].joined(separator: "\n")

        let service = MockMosaicService()
        var blocks = service.testableParseBlocks(from: body, noteId: "mixed")

        XCTAssertEqual(blocks.count, 3)
        XCTAssertEqual(blocks.map(\.id), [headingBid, proseBid, fenceBid])
        XCTAssertEqual(blocks[0].displayText, "# Heading")
        XCTAssertEqual(blocks[1].displayText, "Prose one\nprose two")
        XCTAssertEqual(
            blocks[2].displayText,
            "```query\nstatus:: done\n#literal\n- payload, not a block\n\n  extra-indented payload  \n<!-- bid:\(literalBid) -->\n```"
        )
        XCTAssertEqual(blocks[2].text, "```query")
        XCTAssertTrue(blocks[2].properties.isEmpty)
        XCTAssertTrue(blocks[2].tags.isEmpty)
        XCTAssertFalse(blocks[2].displayText.contains(fenceBid))
        XCTAssertTrue(blocks[2].displayText.contains(literalBid))
        XCTAssertEqual(
            MockMosaicService.stripPropertyLines(blocks[2].displayText),
            blocks[2].displayText
        )
        let inlineProjection = MockMosaicService.splitInlineTags(blocks[2].displayText)
        XCTAssertEqual(inlineProjection.body, blocks[2].displayText)
        XCTAssertTrue(inlineProjection.tags.isEmpty)
        XCTAssertEqual(
            blocks.map { MockMosaicService.engineExactText(of: $0) },
            blocks.map(\.displayText)
        )

        func replacing(_ needle: String, with replacement: String, in block: Block) -> Block {
            let exact = MockMosaicService.engineExactText(of: block)
            let range = (exact as NSString).range(of: needle)
            guard range.location != NSNotFound else {
                XCTFail("Missing splice target: \(needle)")
                return block
            }
            return MockMosaicService.applyFaithfulSplice(
                to: block,
                utf16Offset: range.location,
                utf16DeleteLen: range.length,
                insert: replacement
            ).block
        }
        blocks[0] = replacing("Heading", with: "Heading edited", in: blocks[0])
        blocks[1] = replacing("prose two", with: "prose two edited", in: blocks[1])
        blocks[2] = replacing("status:: done", with: "status:: todo", in: blocks[2])
        let rendered = service.testableRenderBody(from: blocks)
        let reparsed = service.testableParseBlocks(from: rendered, noteId: "mixed")

        XCTAssertEqual(reparsed.count, 3)
        XCTAssertEqual(reparsed.map(\.id), [headingBid, proseBid, fenceBid])
        XCTAssertEqual(reparsed[0].displayText, "# Heading edited")
        XCTAssertEqual(reparsed[1].displayText, "Prose one\nprose two edited")
        XCTAssertEqual(
            reparsed[2].displayText,
            "```query\nstatus:: todo\n#literal\n- payload, not a block\n\n  extra-indented payload  \n<!-- bid:\(literalBid) -->\n```"
        )
        XCTAssertTrue(rendered.contains("- <!-- bid:\(fenceBid) -->\n  ```query"))
    }

    /// `tags:: Issue` is the canonical block-tag form emitted by the
    /// desktop/web engine. iOS should surface it as a visible tag chip,
    /// keep it out of generic properties, and write it back as `tags::`
    /// rather than converting it to an inline `#Issue`.
    func testParseBlocksPromotesTagsPropertyToTagsAndPreservesCanonicalRender() {
        let body = """
        - Figure out finances <!-- bid:6AE83FC1-9EE9-4626-9EFE-58E0D83E7176 -->
          tags:: Issue
          IssueStatus::
        """

        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "2026-06-11")

        XCTAssertEqual(blocks.count, 1)
        XCTAssertEqual(blocks[0].text, "Figure out finances")
        XCTAssertEqual(blocks[0].tags, ["#Issue"])
        XCTAssertFalse(blocks[0].properties.contains { $0.key.lowercased() == "tags" })
        XCTAssertEqual(
            blocks[0].properties.first(where: { $0.key == "IssueStatus" })?.value,
            ""
        )
        XCTAssertEqual(service.testableRenderBody(from: blocks), body)
    }

    /// Regression (build 17 feedback): making a Task duplicated its property
    /// lines — renderBody emitted the body's `status::`/`tags::` lines AND
    /// renderProperties re-emitted them, so each save→render stacked another
    /// copy ("Task / status / tags / status / tags"). renderBody now drops
    /// property-shaped lines from the body (renderProperties is the sole
    /// emitter) and renderProperties dedups by key — so a body already polluted
    /// with DOUBLED properties collapses to one of each, a user-set `priority::`
    /// survives, and a re-render is idempotent (no further accumulation).
    func testRenderBodyCollapsesDuplicatedTaskProperties() {
        let body = """
        - Task <!-- bid:6AE83FC1-9EE9-4626-9EFE-58E0D83E7176 -->
          status:: todo
          tags:: Task
          priority:: high
          status:: todo
          tags:: Task
        """

        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "dup")
        let rendered = service.testableRenderBody(from: blocks)

        func count(_ needle: String) -> Int { rendered.components(separatedBy: needle).count - 1 }
        XCTAssertEqual(count("status:: todo"), 1, "status emitted exactly once: \(rendered)")
        XCTAssertEqual(count("tags:: Task"), 1, "tags emitted exactly once: \(rendered)")
        XCTAssertEqual(count("priority:: high"), 1, "user property survives: \(rendered)")
        XCTAssertEqual(count("Task <!--"), 1, "title bullet rendered once")

        // Idempotent: re-parsing + re-rendering does not re-accumulate.
        let reblocks = service.testableParseBlocks(from: rendered, noteId: "dup")
        XCTAssertEqual(service.testableRenderBody(from: reblocks), rendered)
    }

    /// Blocks separated only by blank lines should get correct line numbers.
    func testParseBlocksWithBlankLines() {
        let body = """
        - Alpha

        - Beta

        - Gamma
        """
        let service = MockMosaicService()
        let blocks = service.testableParseBlocks(from: body, noteId: "gaps-note")
        XCTAssertEqual(blocks.count, 3)
        XCTAssertEqual(blocks[0].lineNumber, 0)
        XCTAssertEqual(blocks[1].lineNumber, 2)
        XCTAssertEqual(blocks[2].lineNumber, 4)
    }

    private func date(_ y: Int, _ m: Int, _ d: Int) -> Date {
        var components = DateComponents()
        components.year = y
        components.month = m
        components.day = d
        components.hour = 12
        components.minute = 0
        components.second = 0
        return Calendar.current.date(from: components)!
    }

    private func writeLocalDaily(id: String, body: String) throws {
        let notesDir = localFixtureNotesDir()
        try FileManager.default.createDirectory(at: notesDir, withIntermediateDirectories: true)
        let content = """
        ---
        title: \(id)
        tags: [daily]
        created: \(id)T00:00:00Z
        ---

        \(body)
        """
        try content.write(to: notesDir.appendingPathComponent("\(id).md"), atomically: true, encoding: .utf8)
    }

    private func resetLocalDailyFixtures(_ ids: [String]) throws {
        let notesDir = localFixtureNotesDir()
        for id in ids {
            let path = notesDir.appendingPathComponent("\(id).md")
            if FileManager.default.fileExists(atPath: path.path) {
                try FileManager.default.removeItem(at: path)
            }
        }
    }

    private func localFixtureNotesDir() -> URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("sync-ios-mosaic")
            .appendingPathComponent("notes")
    }

    // MARK: - Placeholder-authoring gate (2026-06-10 product test)

    /// A daily writeback consisting ONLY of bare empty blocks for a daily
    /// that has never been materialized locally is the editable-row
    /// placeholder state, not user content — authoring it as the note's
    /// first synced state put a stray empty bullet ABOVE the peer's real
    /// content after the fresh-day union (iOS: [empty, dude, empty];
    /// desktop: [dude, empty]).
    func testPlaceholderOnlyFreshDailyIsSuppressed() {
        let placeholder = Block(id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D001", kind: .note, text: "")
        XCTAssertTrue(MockMosaicService.isBarePlaceholder(placeholder))
        XCTAssertTrue(
            MockMosaicService.shouldSuppressPlaceholderAuthoring(
                blocks: [placeholder],
                dailyFileExists: false
            )
        )
        // Vacuously-bare empty list on a fresh daily: nothing to author.
        XCTAssertTrue(
            MockMosaicService.shouldSuppressPlaceholderAuthoring(
                blocks: [],
                dailyFileExists: false
            )
        )
    }

    /// Once a local file exists, an all-bare state is a REAL edit (the
    /// user deleted the last contentful block) and must flow — the gate
    /// only applies to a never-persisted daily.
    func testAllBareStateOnExistingDailyStillPushes() {
        let placeholder = Block(id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D002", kind: .note, text: "")
        XCTAssertFalse(
            MockMosaicService.shouldSuppressPlaceholderAuthoring(
                blocks: [placeholder],
                dailyFileExists: true
            )
        )
    }

    /// Any real content — text, tags, properties, or task state — defeats
    /// the gate even on a fresh daily.
    func testContentfulFreshDailyIsAuthored() {
        let typed = Block(id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D003", kind: .note, text: "hello")
        let placeholder = Block(id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D004", kind: .note, text: "")
        XCTAssertFalse(MockMosaicService.isBarePlaceholder(typed))
        XCTAssertFalse(
            MockMosaicService.shouldSuppressPlaceholderAuthoring(
                blocks: [typed, placeholder],
                dailyFileExists: false
            )
        )
        // A bare-text TASK block is still task state (checkbox) — content.
        let task = Block(id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D005", kind: .task, text: "")
        XCTAssertFalse(MockMosaicService.isBarePlaceholder(task))
        // Tags / properties count as content too.
        let tagged = Block(
            id: "5E0A4E27-0A57-4D6B-9F6F-1B1F58A2D006",
            kind: .note,
            text: "",
            tags: ["#inbox"]
        )
        XCTAssertFalse(MockMosaicService.isBarePlaceholder(tagged))
    }

    // MARK: - Capture insertion (append at the bottom, 2026-06-10)

    /// A daily capture appends AFTER the last contentful block — never at
    /// the top (web parity).
    func testCaptureInsertIndexAppendsAfterContent() {
        let a = Block(id: "a", kind: .note, text: "first")
        let b = Block(id: "b", kind: .task, text: "second")
        XCTAssertEqual(MockMosaicService.captureInsertIndex(in: []), 0)
        XCTAssertEqual(MockMosaicService.captureInsertIndex(in: [a]), 1)
        XCTAssertEqual(MockMosaicService.captureInsertIndex(in: [a, b]), 2)
    }

    /// A trailing run of bare "Add block" placeholders stays the visual
    /// tail of the day: the capture slots in just before it.
    func testCaptureInsertIndexSkipsTrailingPlaceholders() {
        let content = Block(id: "a", kind: .note, text: "real")
        let empty1 = Block(id: "b", kind: .note, text: "")
        let empty2 = Block(id: "c", kind: .note, text: "")
        XCTAssertEqual(
            MockMosaicService.captureInsertIndex(in: [content, empty1]), 1
        )
        XCTAssertEqual(
            MockMosaicService.captureInsertIndex(in: [content, empty1, empty2]), 1
        )
        // A placeholder ABOVE content does not move the index — only the
        // trailing run matters.
        XCTAssertEqual(
            MockMosaicService.captureInsertIndex(in: [empty1, content]), 2
        )
        // All-placeholder day: capture lands first.
        XCTAssertEqual(
            MockMosaicService.captureInsertIndex(in: [empty1, empty2]), 0
        )
    }

    /// End-to-end through `capture(_:target:)` on the in-memory model:
    /// the captured block is appended after the seed's last block, not
    /// inserted at index 0.
    func testCaptureAppendsAtBottomOfToday() {
        let service = MockMosaicService()
        let countBefore = service.todayBlocks.count
        service.capture("captured at the bottom", target: .today)
        XCTAssertEqual(service.todayBlocks.count, countBefore + 1)
        XCTAssertEqual(service.todayBlocks.last?.text, "captured at the bottom")
        XCTAssertNotEqual(service.todayBlocks.first?.text, "captured at the bottom")
        // Inbox captures append too (with the #inbox tag).
        service.capture("inbox capture", target: .inbox)
        XCTAssertEqual(service.todayBlocks.last?.text, "inbox capture")
        XCTAssertEqual(service.todayBlocks.last?.tags, ["#inbox"])
    }

    // MARK: - Capture type picker + add-time inline NLP (2026-06-29)

    /// No type chosen → plain `.note`, text untouched, no tag, no props
    /// (today's behavior is preserved — the picker is the NLP prerequisite).
    func testApplyCaptureTypeNilTagIsPlainNote() {
        let reg = PropertyRegistry.buildBuiltins()
        let r = MockMosaicService.applyCaptureType(
            text: "Test p1 due tomorrow", tag: nil, registry: reg)
        XCTAssertEqual(r.kind, .note)
        XCTAssertEqual(r.body, "Test p1 due tomorrow")
        XCTAssertTrue(r.tags.isEmpty)
        XCTAssertTrue(r.props.isEmpty)
    }

    /// type=Task → `.task` kind, `#Task` tag, and inline NLP lifts the
    /// priority + deadline tokens out of the prose into structured props.
    func testApplyCaptureTypeTaskLiftsPriorityAndDeadline() {
        let reg = PropertyRegistry.buildBuiltins()
        let r = MockMosaicService.applyCaptureType(
            text: "Test p1 due tomorrow", tag: "Task", registry: reg)
        XCTAssertEqual(r.kind, .task)
        XCTAssertEqual(r.tags, ["#Task"])
        XCTAssertEqual(r.props.first(where: { $0.key == "priority" })?.value, "p1")
        XCTAssertNotNil(
            r.props.first(where: { $0.key == "deadline" }),
            "a 'due tomorrow' deadline should lift onto the Task")
        XCTAssertFalse(r.body.lowercased().contains("p1"))
        XCTAssertFalse(r.body.lowercased().contains("tomorrow"))
        XCTAssertTrue(r.body.contains("Test"))
    }

    /// A `#` prefix on the chosen type is tolerated (and not doubled).
    func testApplyCaptureTypeToleratesHashPrefix() {
        let reg = PropertyRegistry.buildBuiltins()
        let r = MockMosaicService.applyCaptureType(
            text: "Plain task", tag: "#Task", registry: reg)
        XCTAssertEqual(r.kind, .task)
        XCTAssertEqual(r.tags, ["#Task"])
    }

    /// A non-Task type stays a `.note`, tags-only, and still lifts a date
    /// the type declares (Project has Deadline). A property the type does
    /// NOT declare (Priority) is not lifted — it stays in the prose.
    func testApplyCaptureTypeProjectLiftsOnlyDeclaredProps() {
        let reg = PropertyRegistry.buildBuiltins()
        let r = MockMosaicService.applyCaptureType(
            text: "Roadmap p1 due tomorrow", tag: "Project", registry: reg)
        XCTAssertEqual(r.kind, .note)
        XCTAssertEqual(r.tags, ["#Project"])
        XCTAssertNotNil(r.props.first(where: { $0.key == "deadline" }))
        XCTAssertNil(
            r.props.first(where: { $0.key == "priority" }),
            "Project doesn't declare Priority, so p1 must not lift")
        XCTAssertTrue(
            r.body.lowercased().contains("p1"),
            "an unlifted token stays in the prose: \(r.body)")
        XCTAssertFalse(r.body.lowercased().contains("tomorrow"))
    }

    /// Plain prose with no NLP triggers → text passes through unchanged
    /// (still tagged with the chosen type, no props).
    func testApplyCaptureTypeNoTriggersLeavesTextUnchanged() {
        let reg = PropertyRegistry.buildBuiltins()
        let r = MockMosaicService.applyCaptureType(
            text: "just a plain task", tag: "Task", registry: reg)
        XCTAssertEqual(r.kind, .task)
        XCTAssertEqual(r.body, "just a plain task")
        XCTAssertEqual(r.tags, ["#Task"])
        XCTAssertTrue(r.props.isEmpty)
    }

    /// Mirror the capture sheet's type picker: it offers the live registry's
    /// type names, falling back to the built-ins (Task/Project) when the
    /// registry hasn't synced yet, so the picker is never empty
    /// (`CaptureBar.captureTypeNames` / `grCaptureTypes`). The `manualTag`
    /// the user picks is one of THESE strings.
    private func pickedCaptureType(_ name: String, on service: MockMosaicService) -> String {
        let live = service.propertyRegistry.typeNames()
        let names = live.isEmpty ? PropertyRegistry.buildBuiltins().typeNames() : live
        return names.first(where: { $0 == name }) ?? name
    }

    /// End-to-end through the EXACT UI path: an UNSYNCED service (registry is
    /// the empty default, mirroring a device before its Property pages have
    /// synced) + a `manualTag` taken from the picker's built-in fallback.
    /// Regression for the build-62 bug: the block got tagged `#Task` but the
    /// inline-NLP tokens were NOT lifted, because `capture` resolved NLP
    /// against the empty live registry while the picker showed the built-in
    /// types. The fix gives the capture path the same built-ins fallback the
    /// picker has, so "p2"/"due tomorrow" lift even on an unsynced registry.
    func testCaptureWithTaskTypeTagsAndLiftsPropsOntoBlock() async {
        let service = MockMosaicService()  // NO refresh: registry is empty
        XCTAssertTrue(
            service.propertyRegistry.typeNames().isEmpty,
            "precondition: unsynced registry resolves no types")
        let tag = pickedCaptureType("Task", on: service)
        service.capture("Ship it p2 due tomorrow", target: .today, tag: tag)
        guard let block = service.todayBlocks.last else {
            return XCTFail("no captured block")
        }
        XCTAssertEqual(block.kind, .task)
        XCTAssertTrue(block.tags.contains("#Task"))
        XCTAssertEqual(
            block.properties.first(where: { $0.key == "priority" })?.value, "p2")
        XCTAssertNotNil(block.properties.first(where: { $0.key == "deadline" }))
        XCTAssertFalse(block.text.lowercased().contains("p2"))
        XCTAssertFalse(block.text.lowercased().contains("tomorrow"))
        XCTAssertTrue(block.text.contains("Ship it"))
    }

    /// The reported repro verbatim: "Ship it p2 tomorrow" + Task. Taylor's
    /// locked decision (2026-06-30): a date phrase TRAILING the text lifts the
    /// type's DEFAULT date property (Deadline for Task) EVEN without a "due"/"on"
    /// intent word — matching how he types "p1 tomorrow". So BOTH lift → text
    /// "Ship it", Priority p2, and a Deadline. Driven through the unsynced UI
    /// path so it exercises the built-ins fallback.
    func testCaptureTaskBareTrailingDateLiftsPriorityAndDeadline() async {
        let service = MockMosaicService()  // NO refresh: registry is empty
        let tag = pickedCaptureType("Task", on: service)
        service.capture("Ship it p2 tomorrow", target: .today, tag: tag)
        guard let block = service.todayBlocks.last else {
            return XCTFail("no captured block")
        }
        XCTAssertEqual(block.kind, .task)
        XCTAssertTrue(block.tags.contains("#Task"))
        XCTAssertEqual(
            block.properties.first(where: { $0.key == "priority" })?.value, "p2")
        XCTAssertNotNil(
            block.properties.first(where: { $0.key == "deadline" }),
            "a bare TRAILING 'tomorrow' lifts the Task's default date (Deadline)")
        XCTAssertFalse(block.text.lowercased().contains("p2"))
        XCTAssertFalse(block.text.lowercased().contains("tomorrow"))
        XCTAssertEqual(block.text, "Ship it")
    }

    /// A date mid-prose (followed by more words) is NOT trailing, so it still
    /// requires an intent word — it does NOT lift. The trailing priority still
    /// lifts. "call her tomorrow about p1" + Task → text keeps "tomorrow",
    /// Priority p1, NO deadline. Scopes the trailing-date rule to limit false
    /// positives.
    func testCaptureTaskMidProseDateStaysProse() async {
        let service = MockMosaicService()  // NO refresh: registry is empty
        let tag = pickedCaptureType("Task", on: service)
        service.capture("call her tomorrow about p1", target: .today, tag: tag)
        guard let block = service.todayBlocks.last else {
            return XCTFail("no captured block")
        }
        XCTAssertEqual(
            block.properties.first(where: { $0.key == "priority" })?.value, "p1")
        XCTAssertNil(
            block.properties.first(where: { $0.key == "deadline" }),
            "a mid-prose 'tomorrow' (more words follow) must not lift a deadline")
        XCTAssertTrue(
            block.text.lowercased().contains("tomorrow"),
            "the mid-prose date stays in the text: \(block.text)")
    }

    /// Capture with no type is byte-for-byte today's plain-note behavior.
    func testCaptureWithoutTypeIsUnchangedPlainNote() async {
        let service = MockMosaicService()
        await service.refresh(from: .mock)
        service.capture("Buy milk p1 due tomorrow", target: .today, tag: nil)
        guard let block = service.todayBlocks.last else {
            return XCTFail("no captured block")
        }
        XCTAssertEqual(block.kind, .note)
        XCTAssertTrue(block.tags.isEmpty)
        XCTAssertTrue(block.properties.isEmpty)
        XCTAssertEqual(block.text, "Buy milk p1 due tomorrow")
    }

    /// An inbox capture WITH a type keeps both the `#inbox` routing tag and
    /// the chosen `#Task` type tag, and still lifts props.
    func testCaptureInboxWithTypeKeepsBothTags() async {
        let service = MockMosaicService()
        await service.refresh(from: .mock)
        service.capture("Triage this p1", target: .inbox, tag: "Task")
        guard let block = service.todayBlocks.last else {
            return XCTFail("no captured block")
        }
        XCTAssertTrue(block.tags.contains("#inbox"))
        XCTAssertTrue(block.tags.contains("#Task"))
        XCTAssertEqual(
            block.properties.first(where: { $0.key == "priority" })?.value, "p1")
    }

    // MARK: - Task toggle write path (2026-06-10 revert fix)

    func testTaskStatusValue() {
        XCTAssertEqual(MockMosaicService.taskStatusValue(done: true), "done")
        XCTAssertEqual(MockMosaicService.taskStatusValue(done: false), "todo")
    }

    /// The toggle flips `done` AND mirrors it into the block's `status`
    /// property — the typed `status::` write is what persists (engine
    /// container op in `.relay` / set-property POST in `.http`); the
    /// in-memory property keeps any later whole-note writeback
    /// consistent with the flip.
    func testToggleTaskFlipsDoneAndStatusProperty() {
        let service = MockMosaicService()
        guard let task = service.todayBlocks.first(where: { $0.kind == .task && !$0.done }) else {
            return XCTFail("seed should contain an open task")
        }
        service.toggleTask(id: task.id)
        let toggled = service.todayBlocks.first(where: { $0.id == task.id })
        XCTAssertEqual(toggled?.done, true)
        XCTAssertEqual(
            toggled?.properties.first(where: { $0.key.lowercased() == "status" })?.value,
            "done"
        )
        // Toggle back: status mirrors to todo.
        service.toggleTask(id: task.id)
        let untoggled = service.todayBlocks.first(where: { $0.id == task.id })
        XCTAssertEqual(untoggled?.done, false)
        XCTAssertEqual(
            untoggled?.properties.first(where: { $0.key.lowercased() == "status" })?.value,
            "todo"
        )
    }

    /// Toggling a non-task block is a no-op (the checkbox only exists on
    /// task rows).
    func testToggleTaskIgnoresNotes() {
        let service = MockMosaicService()
        guard let note = service.todayBlocks.first(where: { $0.kind == .note }) else {
            return XCTFail("seed should contain a note block")
        }
        service.toggleTask(id: note.id)
        let after = service.todayBlocks.first(where: { $0.id == note.id })
        XCTAssertEqual(after?.done, false)
        XCTAssertTrue(after?.properties.isEmpty ?? false)
    }

    // MARK: - Daily feed pagination helpers (2026-06-10)

    func testIsDailySlug() {
        XCTAssertTrue(MockMosaicService.isDailySlug("2026-06-09"))
        XCTAssertTrue(MockMosaicService.isDailySlug("1999-01-31"))
        XCTAssertFalse(MockMosaicService.isDailySlug("tag-system"))
        XCTAssertFalse(MockMosaicService.isDailySlug("2026-6-9"))
        XCTAssertFalse(MockMosaicService.isDailySlug("2026-06-09-extra"))
        XCTAssertFalse(MockMosaicService.isDailySlug(""))
    }

    // MARK: - stripPropertyLines (P5.1 raw-property-line fix)

    func testStripPropertyLinesRemovesTaskPropertyLines() {
        // The exact shape from the field report: prose + folded status::/tags::.
        let merged = "Do this thing\nstatus:: todo\ntags:: Task"
        XCTAssertEqual(MockMosaicService.stripPropertyLines(merged), "Do this thing")
    }

    func testStripPropertyLinesPreservesProseAndTrailingTags() {
        // Prose-only (incl. an inline #tag) is untouched.
        XCTAssertEqual(MockMosaicService.stripPropertyLines("Call mom #family"), "Call mom #family")
        // Multi-line prose continuation survives; only property lines drop.
        let merged = "Line one\nLine two\nscheduled:: [[2026-06-25]]"
        XCTAssertEqual(MockMosaicService.stripPropertyLines(merged), "Line one\nLine two")
    }

    func testStripPropertyLinesIgnoresNonPropertyColons() {
        // A line with no `::` (incl. a URL's `//`) is prose, kept verbatim.
        XCTAssertEqual(MockMosaicService.stripPropertyLines("see https://x.example"), "see https://x.example")
        // A leading `::` with empty key is NOT a property → kept.
        XCTAssertEqual(MockMosaicService.stripPropertyLines(":: orphan"), ":: orphan")
    }
}
