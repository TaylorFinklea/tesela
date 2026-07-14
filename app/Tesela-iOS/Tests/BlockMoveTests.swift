import XCTest
@testable import Tesela

@MainActor
private final class ActivationTestGate {
    private var continuation: CheckedContinuation<Void, Never>?
    private(set) var isBlocked = false

    func wait() async {
        await withCheckedContinuation { continuation in
            self.continuation = continuation
            isBlocked = true
        }
    }

    func waitUntilBlocked() async -> Bool {
        for _ in 0..<1_000 {
            if isBlocked { return true }
            await Task.yield()
        }
        return isBlocked
    }

    func release() {
        isBlocked = false
        let continuation = continuation
        self.continuation = nil
        continuation?.resume()
    }
}

private final class GroupBoundRequestRecorder: @unchecked Sendable {
    struct Entry: Equatable {
        let method: String
        let path: String
        let expectedGroup: String?
    }

    private let lock = NSLock()
    private var entries: [Entry] = []

    func append(_ request: URLRequest) {
        lock.lock()
        entries.append(Entry(
            method: request.httpMethod ?? "GET",
            path: request.url?.path ?? "",
            expectedGroup: request.value(
                forHTTPHeaderField: "X-Tesela-Expected-Group"
            )
        ))
        lock.unlock()
    }

    func snapshot() -> [Entry] {
        lock.lock()
        defer { lock.unlock() }
        return entries
    }
}

private final class GroupMismatchURLProtocol: URLProtocol {
    static var recorder = GroupBoundRequestRecorder()

    override class func canInit(with request: URLRequest) -> Bool {
        request.url?.host == "group-mismatch.test"
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let url = request.url else { return }
        Self.recorder.append(request)
        let data = Data(#"{"error":"mosaic_group_mismatch"}"#.utf8)
        let response = HTTPURLResponse(
            url: url,
            statusCode: 409,
            httpVersion: "HTTP/1.1",
            headerFields: ["Content-Type": "application/json"]
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: data)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

private final class BlockingActivationRefreshURLProtocol: URLProtocol {
    private final class State: @unchecked Sendable {
        private let lock = NSLock()
        private var blockedDaily: BlockingActivationRefreshURLProtocol?
        private var hasBlockedDaily = false
        private var requests: [(method: String, path: String)] = []

        func reset() {
            lock.lock()
            blockedDaily = nil
            hasBlockedDaily = false
            requests = []
            lock.unlock()
        }

        func record(_ request: URLRequest) {
            lock.lock()
            requests.append((request.httpMethod ?? "GET", request.url?.path ?? ""))
            lock.unlock()
        }

        func blockDailyIfNeeded(_ request: BlockingActivationRefreshURLProtocol) -> Bool {
            lock.lock()
            defer { lock.unlock() }
            guard !hasBlockedDaily else { return false }
            hasBlockedDaily = true
            blockedDaily = request
            return true
        }

        func isDailyBlocked() -> Bool {
            lock.lock()
            defer { lock.unlock() }
            return blockedDaily != nil
        }

        func releaseDaily() {
            lock.lock()
            let request = blockedDaily
            blockedDaily = nil
            lock.unlock()
            request?.finishDaily()
        }

        func requestCount(method: String, path: String) -> Int {
            lock.lock()
            defer { lock.unlock() }
            return requests.filter { $0.method == method && $0.path == path }.count
        }
    }

    private static let state = State()

    static func reset() {
        state.reset()
    }

    static func waitUntilDailyIsBlocked() async -> Bool {
        for _ in 0..<2_000 {
            if state.isDailyBlocked() { return true }
            try? await Task.sleep(nanoseconds: 1_000_000)
        }
        return state.isDailyBlocked()
    }

    static func releaseDaily() {
        state.releaseDaily()
    }

    static func requestCount(method: String, path: String) -> Int {
        state.requestCount(method: method, path: path)
    }

    override class func canInit(with request: URLRequest) -> Bool {
        request.url?.host == "activation-race.test"
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        Self.state.record(request)
        if request.url?.path == "/notes/daily", Self.state.blockDailyIfNeeded(self) {
            return
        }
        finishNonblockingRequest()
    }

    override func stopLoading() {}

    private func finishDaily() {
        let body = #"{"id":"2026-07-14","title":"Today","content":"---\ntitle: Today\n---\n\n- existing A block\n","body":"- existing A block\n","metadata":{"tags":[],"custom":{}},"modified_at":"2026-07-14T00:00:00Z"}"#
        finish(status: 200, body: body)
    }

    private func finishNonblockingRequest() {
        let path = request.url?.path ?? ""
        if request.httpMethod == "POST", path == "/blocks/set-property" {
            finish(status: 204, body: "")
        } else if path == "/notes" || path == "/tags" {
            finish(status: 200, body: "[]")
        } else {
            finish(status: 404, body: #"{"error":"not_found"}"#)
        }
    }

    private func finish(status: Int, body: String) {
        guard let url = request.url else { return }
        let response = HTTPURLResponse(
            url: url,
            statusCode: status,
            httpVersion: "HTTP/1.1",
            headerFields: ["Content-Type": "application/json"]
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        if !body.isEmpty {
            client?.urlProtocol(self, didLoad: Data(body.utf8))
        }
        client?.urlProtocolDidFinishLoading(self)
    }
}

@MainActor
final class BlockMoveTests: XCTestCase {
    private let moveId = UUID(uuidString: "51515151-5151-5151-5151-515151515151")!

    func testIntentKeepsOneMoveIdAcrossExactRetries() {
        let intent = BlockMoveIntent(
            moveId: moveId,
            sourceSlug: "2026-07-13",
            rootBid: "61616161-6161-6161-6161-616161616161",
            preview: "parent"
        )
        let destination = BlockMoveDestination(
            slug: "2026-07-12", title: "Yesterday", kind: .daily
        )

        XCTAssertEqual(intent.request(to: destination), intent.request(to: destination))
        XCTAssertEqual(intent.request(to: destination).moveId, moveId.uuidString.lowercased())
    }

    func testDestinationFilterExcludesSourceAndMatchesTitleOrSlug() {
        let destinations = [
            BlockMoveDestination(slug: "source", title: "Source", kind: .page),
            BlockMoveDestination(slug: "project-plan", title: "Project Plan", kind: .page),
            BlockMoveDestination(slug: "2026-07-12", title: "Yesterday", kind: .daily),
        ]

        XCTAssertEqual(
            BlockMoveDestination.filtered(destinations, query: "plan", excluding: "source")
                .map(\.slug),
            ["project-plan"]
        )
        XCTAssertEqual(
            BlockMoveDestination.filtered(destinations, query: "2026-07", excluding: "source")
                .map(\.slug),
            ["2026-07-12"]
        )
    }

    func testServiceForwardsTheExactDurableMoveRequest() async throws {
        let service = MockMosaicService()
        let intent = BlockMoveIntent(
            moveId: moveId,
            sourceSlug: "2026-07-13",
            rootBid: "61616161-6161-6161-6161-616161616161",
            preview: "parent"
        )
        let request = intent.request(to: BlockMoveDestination(
            slug: "2026-07-12", title: "Yesterday", kind: .daily
        ))
        var received: BlockMoveRequest?
        service.onLocalBlockMove = { forwarded in
            received = forwarded
            return [forwarded.sourceSlug, forwarded.destinationSlug]
        }

        try await service.enqueueBackendMutation { reservation in
            try await service.moveSubtree(request, reservation: reservation)
        }.value

        XCTAssertEqual(received, request)
    }

    func testServiceFailureLeavesRenderedSnapshotsUnchanged() async {
        enum Expected: Error { case rejected }
        let service = MockMosaicService()
        let beforeToday = service.todayBlocks
        let beforeYesterday = service.yesterdayBlocks
        service.onLocalBlockMove = { _ in throw Expected.rejected }
        let request = BlockMoveRequest(
            moveId: moveId.uuidString.lowercased(),
            sourceSlug: "2026-07-13",
            rootBid: "61616161-6161-6161-6161-616161616161",
            destinationSlug: "2026-07-12"
        )

        do {
            try await service.enqueueBackendMutation { reservation in
                try await service.moveSubtree(request, reservation: reservation)
            }.value
            XCTFail("expected relocation rejection")
        } catch Expected.rejected {
        } catch {
            XCTFail("unexpected error: \(error)")
        }

        XCTAssertEqual(service.todayBlocks, beforeToday)
        XCTAssertEqual(service.yesterdayBlocks, beforeYesterday)
    }

    func testRelayMoveDestinationsHidePagesThatAreNotMaterialized() {
        let pages = [
            BlockMoveDestination(slug: "local", title: "Local", kind: .page),
            BlockMoveDestination(slug: "index-only", title: "Index only", kind: .page),
        ]

        XCTAssertEqual(
            MockMosaicService.availableBlockMovePages(
                pages,
                backend: .relay,
                isMaterialized: { $0 == "local" }
            ).map(\.slug),
            ["local"]
        )
        XCTAssertEqual(
            MockMosaicService.availableBlockMovePages(
                pages,
                backend: .http(URL(string: "http://127.0.0.1:7474")!),
                isMaterialized: { _ in false }
            ).map(\.slug),
            ["local", "index-only"]
        )
    }

    func testFailedDeliveryCommitsNeitherPreparedBaseline() {
        let existing = ["source": Data([1]), "destination": Data([2])]
        let prepared = PreparedDeltaFrameRecord(
            frame: Data([9]),
            notes: [
                PreparedDeltaNoteRecord(slug: "destination", version: Data([3])),
                PreparedDeltaNoteRecord(slug: "source", version: Data([4])),
            ]
        )

        XCTAssertEqual(
            RelayTicker.baselinesAfterDelivery(
                existing: existing,
                prepared: prepared,
                delivered: false
            ),
            existing
        )
        XCTAssertEqual(
            RelayTicker.baselinesAfterDelivery(
                existing: existing,
                prepared: prepared,
                delivered: true
            ),
            ["source": Data([4]), "destination": Data([3])]
        )
    }

    func testRecoveryRequiredAllowsOnlyExactRetry() {
        XCTAssertTrue(
            BlockMoveSheet.requiresExactRetry(
                FfiSyncError.RelocationRecoveryRequired(
                    moveId: moveId.uuidString.lowercased(),
                    message: "retry"
                )
            )
        )
        XCTAssertFalse(
            BlockMoveSheet.requiresExactRetry(
                FfiSyncError.RelocationRejected(message: "pick another day")
            )
        )
    }

    func testRelocationOutboxPersistsTheExactRequestAndPreparedFrame() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = RelocationOutboxStore(
            url: directory.appendingPathComponent("relocation-outbox.json")
        )
        let request = BlockMoveRequest(
            moveId: moveId.uuidString.lowercased(),
            sourceSlug: "2026-07-13",
            rootBid: "61616161-6161-6161-6161-616161616161",
            destinationSlug: "2026-07-12"
        )
        let prepared = PreparedDeltaFrameRecord(
            frame: Data([7, 8, 9]),
            notes: [
                PreparedDeltaNoteRecord(slug: "2026-07-12", version: Data([1])),
                PreparedDeltaNoteRecord(slug: "2026-07-13", version: Data([2])),
            ]
        )
        let delivery = PendingRelocationDelivery(
            hubIdentity: "http://127.0.0.1:7474",
            request: request,
            prepared: PendingRelocationFrame(prepared),
            engineScope: MosaicEngineScope(
                groupIdHex: "aabbccddeeff00112233445566778899"
            )
        )

        try store.save(delivery)
        XCTAssertEqual(try store.load(), delivery)
        XCTAssertEqual(try store.load()?.prepared?.prepared.frame, prepared.frame)

        try store.clear()
        XCTAssertNil(try store.load())
    }

    func testBuild78RelocationOutboxWithoutEngineScopeDecodesForFailClosedCleanup() throws {
        struct Build78Delivery: Codable {
            let hubIdentity: String
            let request: BlockMoveRequest
            let prepared: PendingRelocationFrame?
        }
        let legacy = Build78Delivery(
            hubIdentity: "http://mac.local:7474|legacy|/mosaics/personal",
            request: BlockMoveRequest(
                moveId: moveId.uuidString.lowercased(),
                sourceSlug: "2026-07-13",
                rootBid: "61616161-6161-6161-6161-616161616161",
                destinationSlug: "2026-07-12"
            ),
            prepared: nil
        )

        let decoded = try JSONDecoder().decode(
            PendingRelocationDelivery.self,
            from: JSONEncoder().encode(legacy)
        )
        XCTAssertNil(decoded.engineScope)
        XCTAssertNotEqual(decoded.engineScope, MosaicEngineScope.legacy)
        XCTAssertEqual(decoded.request, legacy.request)
    }

    func testBuild78RelocationOutboxIsClearedWithoutReplayingIntoActiveGroup() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("relocation-outbox.json")
        let hub = "http://mac.local:7474|legacy|/mosaics/personal"
        try RelocationOutboxStore(url: url).save(PendingRelocationDelivery(
            hubIdentity: hub,
            request: BlockMoveRequest(
                moveId: moveId.uuidString.lowercased(),
                sourceSlug: "2026-07-13",
                rootBid: "61616161-6161-6161-6161-616161616161",
                destinationSlug: "2026-07-12"
            ),
            prepared: nil,
            engineScope: nil
        ))
        let ticker = RelayTicker(relocationOutboxURL: url)
        ticker.hubMode = true
        ticker.configureLiveHub(identity: hub)

        let didClear = await ticker.retryPendingRelocation()
        XCTAssertTrue(didClear)
        XCTAssertFalse(ticker.hasPendingRelocation)
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
        XCTAssertTrue(ticker.lastError?.contains("older build") == true)
    }

    func testSessionHelloRequiresTheExpectedExactMosaicPathAndGroup() {
        let expected = "/Users/taylor/mosaics/work"
        let group = "aabbccddeeff00112233445566778899"
        let accepted = "{\"event\":\"loro_session\",\"mosaic_path\":\"/Users/taylor/mosaics/work\",\"group_id_hex\":\"\(group)\"}"

        XCTAssertEqual(LiveSyncSocket.decodeSessionHello(accepted)?.mosaicPath, expected)
        XCTAssertEqual(LiveSyncSocket.decodeSessionHello(accepted)?.groupIdHex, group)
        XCTAssertTrue(LiveSyncSocket.sessionHelloMatches(
            accepted,
            expectedMosaicPath: expected,
            expectedGroupIdHex: group
        ))
        XCTAssertFalse(LiveSyncSocket.sessionHelloMatches(
            "{\"event\":\"loro_session\",\"mosaic_path\":\"/Users/taylor/mosaics/personal\",\"group_id_hex\":\"\(group)\"}",
            expectedMosaicPath: expected,
            expectedGroupIdHex: group
        ))
        XCTAssertFalse(LiveSyncSocket.sessionHelloMatches(
            "{\"event\":\"loro_session\",\"mosaic_path\":\"/Users/taylor/mosaics/work\",\"group_id_hex\":\"bbbbccddeeff00112233445566778899\"}",
            expectedMosaicPath: expected,
            expectedGroupIdHex: group
        ))
        XCTAssertFalse(LiveSyncSocket.sessionHelloMatches(
            "{\"event\":\"loro_session\"}",
            expectedMosaicPath: expected,
            expectedGroupIdHex: group
        ))
    }

    func testConnectionBindingSeparatesMosaicPathAndClientHubIdentity() {
        let a = LiveSyncSocket.connectionBinding(
            serverURL: "HTTP://mac.local:7474/",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: "aa11",
            hubIdentity: "profile-a"
        )
        XCTAssertEqual(a, LiveSyncSocket.connectionBinding(
            serverURL: "http://mac.local:7474",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: "AA11",
            hubIdentity: "profile-a"
        ))
        XCTAssertNotEqual(a, LiveSyncSocket.connectionBinding(
            serverURL: "http://mac.local:7474",
            expectedMosaicPath: "/mosaics/b",
            expectedGroupIdHex: "aa11",
            hubIdentity: "profile-a"
        ))
        XCTAssertNotEqual(a, LiveSyncSocket.connectionBinding(
            serverURL: "http://mac.local:7474",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: "bb22",
            hubIdentity: "profile-a"
        ))
        XCTAssertNotEqual(a, LiveSyncSocket.connectionBinding(
            serverURL: "http://mac.local:7474",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: "aa11",
            hubIdentity: "profile-b"
        ))
        let secure = LiveSyncSocket.connectionBinding(
            serverURL: "HTTPS://mac.local:7474/",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: "aa11",
            hubIdentity: "profile-a"
        )
        XCTAssertEqual(secure, LiveSyncSocket.connectionBinding(
            serverURL: "https://mac.local:7474",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: "aa11",
            hubIdentity: "profile-a"
        ))
        XCTAssertEqual(secure?.websocketURL.scheme, "wss")
    }

    func testBarrierAcknowledgementRequiresTheExpectedEventUUIDPathAndGroup() {
        let id = "11111111-1111-4111-8111-111111111111"
        let group = "aabbccddeeff00112233445566778899"
        let accepted = LiveSyncSocket.decodeBarrierAcknowledgement(
            "{\"event\":\"loro_barrier_ack\",\"barrier_id\":\"\(id)\",\"ok\":true,\"mosaic_path\":\"/mosaics/a\",\"group_id_hex\":\"\(group)\"}",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: group
        )

        XCTAssertEqual(accepted?.id.uuidString.lowercased(), id)
        XCTAssertEqual(accepted?.ok, true)
        XCTAssertNil(LiveSyncSocket.decodeBarrierAcknowledgement(
            "{\"event\":\"note_updated\",\"barrier_id\":\"\(id)\",\"ok\":true,\"mosaic_path\":\"/mosaics/a\",\"group_id_hex\":\"\(group)\"}",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: group
        ))
        XCTAssertNil(LiveSyncSocket.decodeBarrierAcknowledgement(
            "{\"event\":\"loro_barrier_ack\",\"barrier_id\":\"not-a-uuid\",\"ok\":true,\"mosaic_path\":\"/mosaics/a\",\"group_id_hex\":\"\(group)\"}",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: group
        ))
        XCTAssertNil(LiveSyncSocket.decodeBarrierAcknowledgement(
            "{\"event\":\"loro_barrier_ack\",\"barrier_id\":\"\(id)\",\"ok\":true}",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: group
        ))
        XCTAssertNil(LiveSyncSocket.decodeBarrierAcknowledgement(
            "{\"event\":\"loro_barrier_ack\",\"barrier_id\":\"\(id)\",\"ok\":true,\"mosaic_path\":\"/mosaics/b\",\"group_id_hex\":\"\(group)\"}",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: group
        ))
        XCTAssertNil(LiveSyncSocket.decodeBarrierAcknowledgement(
            "{\"event\":\"loro_barrier_ack\",\"barrier_id\":\"\(id)\",\"ok\":true,\"mosaic_path\":\"/mosaics/a\",\"group_id_hex\":\"bbbbccddeeff00112233445566778899\"}",
            expectedMosaicPath: "/mosaics/a",
            expectedGroupIdHex: group
        ))
    }

    func testActivationVerificationSucceedsOnlyForTheVerifiedSessionIdentity() {
        XCTAssertTrue(LiveSyncSocket.activationVerificationSucceeded(
            sessionVerified: true,
            expectedMosaicPath: "/mosaics/work",
            expectedGroupIdHex: "aabbccddeeff00112233445566778899",
            acknowledgedMosaicPath: "/mosaics/work",
            acknowledgedGroupIdHex: "AABBCCDDEEFF00112233445566778899",
            delivered: true
        ))
    }

    func testActivationVerificationRejectsMismatchDisconnectAndNegativeBarrier() {
        let expectedPath = "/mosaics/work"
        let expectedGroup = "aabbccddeeff00112233445566778899"
        XCTAssertFalse(LiveSyncSocket.activationVerificationSucceeded(
            sessionVerified: true,
            expectedMosaicPath: expectedPath,
            expectedGroupIdHex: expectedGroup,
            acknowledgedMosaicPath: "/mosaics/personal",
            acknowledgedGroupIdHex: expectedGroup,
            delivered: true
        ))
        XCTAssertFalse(LiveSyncSocket.activationVerificationSucceeded(
            sessionVerified: true,
            expectedMosaicPath: expectedPath,
            expectedGroupIdHex: expectedGroup,
            acknowledgedMosaicPath: expectedPath,
            acknowledgedGroupIdHex: "bbbbccddeeff00112233445566778899",
            delivered: true
        ))
        XCTAssertFalse(LiveSyncSocket.activationVerificationSucceeded(
            sessionVerified: false,
            expectedMosaicPath: expectedPath,
            expectedGroupIdHex: expectedGroup,
            acknowledgedMosaicPath: expectedPath,
            acknowledgedGroupIdHex: expectedGroup,
            delivered: true
        ))
        XCTAssertFalse(LiveSyncSocket.activationVerificationSucceeded(
            sessionVerified: true,
            expectedMosaicPath: expectedPath,
            expectedGroupIdHex: expectedGroup,
            acknowledgedMosaicPath: expectedPath,
            acknowledgedGroupIdHex: expectedGroup,
            delivered: false
        ))
    }

    func testAttachedHTTPRequestFactoryCarriesTheActivePhysicalGroup() {
        let expectedGroup = "aabbccddeeff00112233445566778899"
        let request = MockMosaicService.groupBoundRequest(
            url: URL(string: "http://mac.test/notes")!,
            engineScope: MosaicEngineScope(groupIdHex: expectedGroup.uppercased())
        )

        XCTAssertEqual(
            request.value(forHTTPHeaderField: "X-Tesela-Expected-Group"),
            expectedGroup
        )
        XCTAssertNil(MockMosaicService.groupBoundRequest(
            url: URL(string: "http://mac.test/notes")!,
            engineScope: .legacy
        ).value(forHTTPHeaderField: "X-Tesela-Expected-Group"))
    }

    func testEveryAttachedHTTPCallUsesTheGroupBoundRequestFactory() throws {
        let projectRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: projectRoot.appendingPathComponent(
                "Sources/Data/MockMosaicService.swift"
            ),
            encoding: .utf8
        )

        XCTAssertEqual(
            source.components(separatedBy: "URLRequest(url:").count - 1,
            1,
            "all attached HTTP paths, including refresh, snapshot, upload, and mutations, must use the one group-bound factory"
        )
    }

    func testActivationRefreshSnapshotAndMutationRejectAReplacedGroup() async throws {
        let expectedGroup = "aabbccddeeff00112233445566778899"
        GroupMismatchURLProtocol.recorder = GroupBoundRequestRecorder()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [GroupMismatchURLProtocol.self]
        let service = MockMosaicService(
            session: URLSession(configuration: configuration)
        )
        service.attach(
            backend: .http(URL(string: "http://group-mismatch.test")!),
            engineScope: MosaicEngineScope(groupIdHex: expectedGroup)
        )

        let refreshSucceeded = await service.refreshAttachedBackend()
        XCTAssertFalse(
            refreshSucceeded,
            "a 409 group rejection must fail the activation refresh"
        )
        do {
            _ = try await service.fetchLoroSnapshot(slug: "2026-07-14")
            XCTFail("the stale-group snapshot request must fail")
        } catch {
            // Expected HTTP 409.
        }
        do {
            try await service.enqueueBackendMutation { reservation in
                try await service.setBlockProperty(
                    blockId: "2026-07-14:block-a",
                    key: "status",
                    value: "done",
                    reservation: reservation
                )
            }.value
            XCTFail("the stale-group mutation must fail")
        } catch {
            // Expected HTTP 409.
        }

        let requests = GroupMismatchURLProtocol.recorder.snapshot()
        XCTAssertTrue(requests.contains {
            $0.method == "GET" && $0.path == "/notes/daily"
        })
        XCTAssertTrue(requests.contains {
            $0.method == "GET" && $0.path == "/loro/notes/2026-07-14/snapshot"
        })
        XCTAssertTrue(requests.contains {
            $0.method == "POST" && $0.path == "/blocks/set-property"
        })
        XCTAssertTrue(requests.allSatisfy { $0.expectedGroup == expectedGroup })
    }

    func testPublishedLiveBindingInvalidationCallbackIsOnceOnly() {
        let state = LiveSyncSocket.BindingInvalidationState()
        let binding = LiveSyncSocket.ConnectionBinding(
            websocketURL: URL(string: "ws://mac.test/ws")!,
            expectedMosaicPath: "/mosaics/work",
            expectedGroupIdHex: "aabbccddeeff00112233445566778899",
            hubIdentity: "work"
        )
        var invalidations = 0
        state.onInvalidated = { invalidations += 1 }

        XCTAssertNil(
            state.takeInvalidationCallback(for: binding),
            "activation-time identity failure has no published lease to invalidate"
        )
        state.publish(binding)
        state.takeInvalidationCallback(for: binding)?()
        XCTAssertEqual(invalidations, 1)
        XCTAssertNil(
            state.takeInvalidationCallback(for: binding),
            "one lost binding must not create a retry callback storm"
        )
    }

    func testBothShellsDetachAndReactivateWhenPublishedBindingIsLost() throws {
        let projectRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        for (relativePath, reactivation) in [
            ("Sources/Views/AppShell.swift", "requestMosaicActivation()"),
            ("Sources/Graphite/Shell/GrAppShell.swift", "requestBackendActivation()"),
        ] {
            let source = try String(
                contentsOf: projectRoot.appendingPathComponent(relativePath),
                encoding: .utf8
            )
            let callback = try XCTUnwrap(source.range(
                of: "liveSync.onBindingInvalidated ="
            ))
            let tail = String(source[callback.lowerBound...].prefix(400))
            XCTAssertTrue(tail.contains("suspendCurrentHub()"), relativePath)
            XCTAssertTrue(tail.contains(reactivation), relativePath)
        }
    }

    func testBothShellsVerifyTheDetachedHTTPHubBeforePublishingObservedState() throws {
        let projectRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        for (relativePath, activationName, nextFunction) in [
            ("Sources/Views/AppShell.swift", "activateMosaic", "waitForActivationAdmission"),
            ("Sources/Graphite/Shell/GrAppShell.swift", "activateBackend", "waitForActivationAdmission"),
        ] {
            let source = try String(
                contentsOf: projectRoot.appendingPathComponent(relativePath),
                encoding: .utf8
            )
            let start = try XCTUnwrap(source.range(
                of: "private func \(activationName)"
            ))
            let end = try XCTUnwrap(source.range(
                of: "private func \(nextFunction)",
                range: start.upperBound..<source.endIndex
            ))
            let activation = String(source[start.lowerBound..<end.lowerBound])
            let engine = try XCTUnwrap(activation.range(of: "relayTicker.activateEngine"))
            let verification = try XCTUnwrap(activation.range(of: "liveSync.connectAndVerify"))
            let attach = try XCTUnwrap(activation.range(of: "mosaic.attach"))
            let refresh = try XCTUnwrap(activation.range(of: "mosaic.refreshAttachedBackend"))
            let bootstrap = try XCTUnwrap(activation.range(of: "relayTicker.bootstrapNoteIfNeeded"))

            XCTAssertLessThan(engine.lowerBound, verification.lowerBound, relativePath)
            XCTAssertLessThan(verification.lowerBound, attach.lowerBound, relativePath)
            XCTAssertLessThan(verification.lowerBound, refresh.lowerBound, relativePath)
            XCTAssertLessThan(verification.lowerBound, bootstrap.lowerBound, relativePath)
            XCTAssertTrue(activation.contains("liveSync.disconnect()"), relativePath)
        }
    }

    func testSwitchConfirmationAcceptsOnlyTheExactTargetPath() {
        XCTAssertEqual(
            MockMosaicService.confirmedServerMosaicPath(
                observedPath: "/mosaics/work",
                targetPath: "/mosaics/work"
            ),
            "/mosaics/work"
        )
        XCTAssertNil(MockMosaicService.confirmedServerMosaicPath(
            observedPath: "/mosaics/personal",
            targetPath: "/mosaics/work"
        ))
        XCTAssertNil(MockMosaicService.confirmedServerMosaicPath(
            observedPath: nil,
            targetPath: "/mosaics/work"
        ))
    }

    func testCanonicalEquivalentSwitchDoesNotRestartTheServer() {
        XCTAssertFalse(MockMosaicService.serverMosaicNeedsRestart(
            servingPath: "/Users/taylor/mosaics/work",
            canonicalTargetPath: "/Users/taylor/mosaics/work"
        ))
        XCTAssertTrue(MockMosaicService.serverMosaicNeedsRestart(
            servingPath: "/Users/taylor/mosaics/personal",
            canonicalTargetPath: "/Users/taylor/mosaics/work"
        ))
    }

    func testDetachedServiceRefusesForegroundRefreshUntilActivationCommits() async {
        let service = MockMosaicService()
        XCTAssertFalse(service.todayBlocks.isEmpty)

        service.detachForActivation()

        let detachedRefresh = await service.refreshAttachedBackend()
        XCTAssertFalse(detachedRefresh)
        XCTAssertTrue(service.todayBlocks.isEmpty)

        service.attach(backend: .mock)
        let attachedRefresh = await service.refreshAttachedBackend()
        XCTAssertTrue(attachedRefresh)
        XCTAssertFalse(service.todayBlocks.isEmpty)
    }

    func testActivationRequestClosesNewMutationAdmissionWhileAttachedRefreshIsBlocked() async throws {
        BlockingActivationRefreshURLProtocol.reset()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [BlockingActivationRefreshURLProtocol.self]
        let service = MockMosaicService(
            session: URLSession(configuration: configuration)
        )
        service.attach(backend: .http(URL(string: "http://activation-race.test")!))

        let refresh = Task { @MainActor in
            await service.refreshAttachedBackend()
        }
        let refreshIsBlocked = await BlockingActivationRefreshURLProtocol.waitUntilDailyIsBlocked()
        XCTAssertTrue(refreshIsBlocked)

        let priorWriteGate = ActivationTestGate()
        let priorWrite = service.enqueueBackendMutation { reservation in
            await priorWriteGate.wait()
            try await service.setBlockProperty(
                blockId: "2026-07-14:existing-a",
                key: "status",
                value: "done",
                reservation: reservation
            )
        }
        let priorWriteIsBlocked = await priorWriteGate.waitUntilBlocked()
        XCTAssertTrue(priorWriteIsBlocked)

        service.closeBackendMutationAdmissionForActivation()

        var newWriteRan = false
        let newWrite = service.enqueueBackendMutation { _ in
            newWriteRan = true
        }
        do {
            try await newWrite.value
            XCTFail("a mutation requested after profile B must be rejected")
        } catch is CancellationError {
            // Expected: profile B closed admission synchronously.
        } catch {
            XCTFail("unexpected error: \(error)")
        }
        XCTAssertFalse(newWriteRan)

        priorWriteGate.release()
        try await priorWrite.value
        XCTAssertEqual(
            BlockingActivationRefreshURLProtocol.requestCount(
                method: "POST",
                path: "/blocks/set-property"
            ),
            1,
            "the already-reserved profile-A write must drain instead of being cancelled"
        )

        BlockingActivationRefreshURLProtocol.releaseDaily()
        let refreshSucceeded = await refresh.value
        XCTAssertTrue(refreshSucceeded)
    }

    func testActivationAdmissionRejectsOptimisticEditsUntilCommit() {
        let service = MockMosaicService()
        let before = service.todayBlocks
        let block = try? XCTUnwrap(before.first)

        service.closeBackendMutationAdmissionForActivation()
        if let block {
            service.editTodayBlock(id: block.id, text: "must not edit stale A")
        }

        XCTAssertEqual(service.todayBlocks, before)
        service.commitBackendMutationAdmission()
        XCTAssertTrue(service.backendMutationAdmissionIsOpen)
    }

    func testBothShellsCloseAdmissionBeforeQueuingActivationAndCommitItLast() throws {
        let projectRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        for (relativePath, requestName, activationName) in [
            ("Sources/Views/AppShell.swift", "requestMosaicActivation", "activateMosaic"),
            ("Sources/Graphite/Shell/GrAppShell.swift", "requestBackendActivation", "activateBackend"),
        ] {
            let source = try String(
                contentsOf: projectRoot.appendingPathComponent(relativePath),
                encoding: .utf8
            )
            let requestStart = try XCTUnwrap(source.range(of: "private func \(requestName)"))
            let requestEnd = try XCTUnwrap(source.range(
                of: "private func",
                range: requestStart.upperBound..<source.endIndex
            ))
            let request = String(source[requestStart.lowerBound..<requestEnd.lowerBound])
            let close = try XCTUnwrap(request.range(
                of: "mosaic.closeBackendMutationAdmissionForActivation()"
            ))
            let queue = try XCTUnwrap(request.range(of: "hubActivation.request"))
            XCTAssertLessThan(close.lowerBound, queue.lowerBound, relativePath)

            let activationStart = try XCTUnwrap(source.range(of: "private func \(activationName)"))
            let activationEnd = try XCTUnwrap(source.range(
                of: "private func waitForActivationAdmission",
                range: activationStart.upperBound..<source.endIndex
            ))
            let activation = String(source[activationStart.lowerBound..<activationEnd.lowerBound])
            let refresh = try XCTUnwrap(activation.range(of: "mosaic.refreshAttachedBackend"))
            let commit = try XCTUnwrap(activation.range(of: "mosaic.commitBackendMutationAdmission()"))
            XCTAssertLessThan(refresh.lowerBound, commit.lowerBound, relativePath)
        }
    }

    func testMoveSheetRejectsClosedAdmissionBeforeStartingSpinner() throws {
        let projectRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: projectRoot.appendingPathComponent("Sources/Views/BlockMoveSheet.swift"),
            encoding: .utf8
        )
        let start = try XCTUnwrap(source.range(
            of: "private func perform(_ request: BlockMoveRequest)"
        ))
        let end = try XCTUnwrap(source.range(
            of: "static func requiresExactRetry",
            range: start.upperBound..<source.endIndex
        ))
        let perform = String(source[start.lowerBound..<end.lowerBound])
        let admission = try XCTUnwrap(perform.range(
            of: "guard mosaic.backendMutationAdmissionIsOpen else"
        ))
        let spinner = try XCTUnwrap(perform.range(of: "isMoving = true"))

        XCTAssertLessThan(admission.lowerBound, spinner.lowerBound)
    }

    func testHubIdentitySeparatesProfilesAndMosaicPathsOnOneServer() {
        let profileA = UUID(uuidString: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")!
        let profileB = UUID(uuidString: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb")!

        let a = RelayTicker.hubIdentity(
            serverURL: "HTTP://mac.local:7474/",
            profileID: profileA,
            mosaicPath: "/mosaics/a",
            groupIdHex: "AA11"
        )
        XCTAssertEqual(a, RelayTicker.hubIdentity(
            serverURL: "http://mac.local:7474",
            profileID: profileA,
            mosaicPath: "/mosaics/a",
            groupIdHex: "aa11"
        ))
        XCTAssertNotEqual(a, RelayTicker.hubIdentity(
            serverURL: "http://mac.local:7474",
            profileID: profileA,
            mosaicPath: "/mosaics/b",
            groupIdHex: "aa11"
        ))
        XCTAssertNotEqual(a, RelayTicker.hubIdentity(
            serverURL: "http://mac.local:7474",
            profileID: profileB,
            mosaicPath: "/mosaics/a",
            groupIdHex: "aa11"
        ))
        XCTAssertNotEqual(a, RelayTicker.hubIdentity(
            serverURL: "http://mac.local:7474",
            profileID: profileA,
            mosaicPath: "/mosaics/a",
            groupIdHex: "bb22"
        ))
    }

    func testGraphiteActivationUsesTheActiveProfileServerAndMosaicPath() {
        let profile = MosaicProfile(
            name: "Work",
            serverURL: "https://work-mac.example:7474",
            mosaicPath: "/Users/taylor/mosaics/work"
        )

        let destination = GrAppShell.resolvedHubDestination(
            activeProfile: profile,
            fallbackServerURL: "http://old-mac.local:7474"
        )

        XCTAssertEqual(destination.serverURL, profile.serverURL)
        XCTAssertEqual(destination.mosaicPath, profile.mosaicPath)
    }

    func testHubActivationSerializesPassesAndOnlyLatestCommits() async {
        let sequencer = HubActivationSequencer()
        let gate = ActivationTestGate()
        var active = 0
        var maxActive = 0
        var executed: [String] = []
        var committed: [String] = []

        sequencer.request { lease in
            active += 1
            maxActive = max(maxActive, active)
            executed.append("A")
            await gate.wait()
            if sequencer.isCurrent(lease) {
                committed.append("A")
            }
            active -= 1
        }
        let didBlock = await gate.waitUntilBlocked()
        XCTAssertTrue(didBlock)

        sequencer.request { lease in
            active += 1
            maxActive = max(maxActive, active)
            executed.append("B")
            if sequencer.isCurrent(lease) {
                committed.append("B")
            }
            active -= 1
        }
        gate.release()
        await sequencer.waitUntilIdle()

        XCTAssertEqual(maxActive, 1)
        XCTAssertEqual(executed, ["A", "B"])
        XCTAssertEqual(committed, ["B"])
    }

    func testHubActivationCoalescesPendingRequestsToNewest() async {
        let sequencer = HubActivationSequencer()
        let gate = ActivationTestGate()
        var executed: [String] = []
        var committed: [String] = []

        sequencer.request { lease in
            executed.append("A")
            await gate.wait()
            if sequencer.isCurrent(lease) {
                committed.append("A")
            }
        }
        let didBlock = await gate.waitUntilBlocked()
        XCTAssertTrue(didBlock)

        sequencer.request { lease in
            executed.append("B")
            if sequencer.isCurrent(lease) {
                committed.append("B")
            }
        }
        sequencer.request { lease in
            executed.append("C")
            if sequencer.isCurrent(lease) {
                committed.append("C")
            }
        }
        gate.release()
        await sequencer.waitUntilIdle()

        XCTAssertEqual(executed, ["A", "C"])
        XCTAssertEqual(committed, ["C"])
    }

    func testHubActivationRevisionRejectsStaleABARequest() async {
        let sequencer = HubActivationSequencer()
        let gate = ActivationTestGate()
        var executed: [String] = []
        var committed: [String] = []

        sequencer.request { lease in
            executed.append("old-A")
            await gate.wait()
            if sequencer.isCurrent(lease) {
                committed.append("old-A")
            }
        }
        let didBlock = await gate.waitUntilBlocked()
        XCTAssertTrue(didBlock)

        sequencer.request { lease in
            executed.append("B")
            if sequencer.isCurrent(lease) {
                committed.append("B")
            }
        }
        sequencer.request { lease in
            executed.append("new-A")
            if sequencer.isCurrent(lease) {
                committed.append("new-A")
            }
        }
        gate.release()
        await sequencer.waitUntilIdle()

        XCTAssertEqual(executed, ["old-A", "new-A"])
        XCTAssertEqual(committed, ["new-A"])
    }

    func testHubActivationCanInvalidateARunningLeaseBeforeReplacementIsQueued() async {
        let sequencer = HubActivationSequencer()
        let gate = ActivationTestGate()
        var committed = false

        sequencer.request { lease in
            await gate.wait()
            committed = sequencer.isCurrent(lease)
        }
        let didBlock = await gate.waitUntilBlocked()
        XCTAssertTrue(didBlock)

        sequencer.invalidateCurrentRequest()
        gate.release()
        await sequencer.waitUntilIdle()

        XCTAssertFalse(committed)
    }

    func testRegistrySignalsBeforePublishingANewActiveProfile() {
        let defaults = UserDefaults.standard
        let profilesKey = "mosaics.profiles.v1"
        let activeKey = "mosaics.activeID.v1"
        let savedProfiles = defaults.data(forKey: profilesKey)
        let savedActive = defaults.string(forKey: activeKey)
        defaults.removeObject(forKey: profilesKey)
        defaults.removeObject(forKey: activeKey)
        defer {
            if let savedProfiles {
                defaults.set(savedProfiles, forKey: profilesKey)
            } else {
                defaults.removeObject(forKey: profilesKey)
            }
            if let savedActive {
                defaults.set(savedActive, forKey: activeKey)
            } else {
                defaults.removeObject(forKey: activeKey)
            }
        }

        let registry = MosaicRegistry()
        let profileA = MosaicProfile(name: "A", serverURL: "http://a.test")
        let profileB = MosaicProfile(name: "B", serverURL: "http://b.test")
        registry.add(profileA)
        registry.add(profileB, makeActive: false)
        var observedBeforePublication: UUID?
        registry.willChangeActiveProfile = {
            observedBeforePublication = registry.activeID
        }

        registry.setActive(profileB.id)

        XCTAssertEqual(observedBeforePublication, profileA.id)
        XCTAssertEqual(registry.activeID, profileB.id)
    }

    func testBothShellsInvalidateActivationFromRegistryPreChangeHook() throws {
        let projectRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        for relativePath in [
            "Sources/Views/AppShell.swift",
            "Sources/Graphite/Shell/GrAppShell.swift",
        ] {
            let source = try String(
                contentsOf: projectRoot.appendingPathComponent(relativePath),
                encoding: .utf8
            )
            let hook = try XCTUnwrap(source.range(
                of: "mosaicRegistry.willChangeActiveProfile ="
            ))
            let body = String(source[hook.lowerBound...].prefix(400))
            XCTAssertTrue(
                body.contains("mosaic?.closeBackendMutationAdmissionForActivation()"),
                relativePath
            )
            XCTAssertTrue(
                body.contains("hubActivation?.invalidateCurrentRequest()"),
                relativePath
            )
        }
    }

    func testHubActivationWaitsForMoveStagingAndBootstrap() {
        XCTAssertFalse(RelayTicker.isHubActivationSafe(
            engineOperationsInFlight: 0,
            relocationInFlight: true,
            bootstrapInFlight: 0,
            pendingPrepared: nil,
            hasLiveHubIdentity: true
        ))
        XCTAssertFalse(RelayTicker.isHubActivationSafe(
            engineOperationsInFlight: 0,
            relocationInFlight: false,
            bootstrapInFlight: 1,
            pendingPrepared: nil,
            hasLiveHubIdentity: true
        ))
        XCTAssertFalse(RelayTicker.isHubActivationSafe(
            engineOperationsInFlight: 0,
            relocationInFlight: false,
            bootstrapInFlight: 0,
            pendingPrepared: false,
            hasLiveHubIdentity: true
        ))
        XCTAssertTrue(RelayTicker.isHubActivationSafe(
            engineOperationsInFlight: 0,
            relocationInFlight: false,
            bootstrapInFlight: 0,
            pendingPrepared: true,
            hasLiveHubIdentity: true
        ))
        XCTAssertTrue(RelayTicker.isHubActivationSafe(
            engineOperationsInFlight: 0,
            relocationInFlight: false,
            bootstrapInFlight: 0,
            pendingPrepared: false,
            hasLiveHubIdentity: false
        ))
        XCTAssertFalse(RelayTicker.isHubActivationSafe(
            engineOperationsInFlight: 1,
            relocationInFlight: false,
            bootstrapInFlight: 0,
            pendingPrepared: true,
            hasLiveHubIdentity: true
        ))
        XCTAssertFalse(RelayTicker.isHubActivationSafe(
            engineOperationsInFlight: 0,
            engineActivationInFlight: true,
            relocationInFlight: false,
            bootstrapInFlight: 0,
            pendingPrepared: true,
            hasLiveHubIdentity: true
        ))
        XCTAssertFalse(RelayTicker.isHubActivationSafe(
            engineOperationsInFlight: 0,
            relayOperationInFlight: true,
            relocationInFlight: false,
            bootstrapInFlight: 0,
            pendingPrepared: true,
            hasLiveHubIdentity: true
        ))
        XCTAssertFalse(RelayTicker.isHubActivationSafe(
            engineOperationsInFlight: 0,
            backgroundFlushInFlight: true,
            relocationInFlight: false,
            bootstrapInFlight: 0,
            pendingPrepared: true,
            hasLiveHubIdentity: true
        ))
    }

    func testEngineScopeUsesPhysicalGroupIdentityAcrossTransports() {
        let documents = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let upper = MosaicEngineScope(groupIdHex: "AABBCCDDEEFF00112233445566778899")
        let lower = MosaicEngineScope(groupIdHex: "aabbccddeeff00112233445566778899")
        let other = MosaicEngineScope(groupIdHex: "bbbbccddeeff00112233445566778899")

        XCTAssertEqual(upper, lower)
        XCTAssertEqual(upper.rootURL(documentsURL: documents), lower.rootURL(documentsURL: documents))
        XCTAssertNotEqual(upper.rootURL(documentsURL: documents), other.rootURL(documentsURL: documents))
        XCTAssertNotEqual(upper.rootURL(documentsURL: documents), MosaicEngineScope.legacy.rootURL(documentsURL: documents))
    }

    func testSandboxSnapshotWriterRejectsAStaleActivationGeneration() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let writer = SandboxSnapshotWriter()
        let note = SandboxNoteSnapshot(
            id: "daily",
            content: "new generation",
            modifiedAt: .distantFuture
        )

        writer.activate(generation: 1)
        writer.activate(generation: 2)
        writer.write(note, to: directory, generation: 1)
        let path = directory.appendingPathComponent("daily.md")
        XCTAssertFalse(FileManager.default.fileExists(atPath: path.path))

        writer.write(note, to: directory, generation: 2)
        XCTAssertEqual(try String(contentsOf: path, encoding: .utf8), "new generation")
    }

    func testEngineSessionTokenRejectsAnOldTaskAfterHubABA() {
        let ticker = RelayTicker(
            relocationOutboxURL: FileManager.default.temporaryDirectory
                .appendingPathComponent(UUID().uuidString)
        )
        ticker.hubMode = true
        ticker.configureLiveHub(identity: "hub-A")
        let oldA = ticker.engineSessionToken

        ticker.configureLiveHub(identity: nil)
        ticker.configureLiveHub(identity: "hub-B")
        ticker.configureLiveHub(identity: nil)
        ticker.configureLiveHub(identity: "hub-A")
        let newA = ticker.engineSessionToken

        XCTAssertEqual(oldA.scope, newA.scope)
        XCTAssertEqual(oldA.hubIdentity, newA.hubIdentity)
        XCTAssertNotEqual(oldA.generation, newA.generation)
        XCTAssertFalse(RelayTicker.isEngineSessionCurrent(required: oldA, current: newA))
    }

    func testMoveSheetGenerationLeaseRejectsAQueuedMoveAfterReattach() async {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        let gate = ActivationTestGate()
        var relocationCalls = 0
        service.onLocalBlockMove = { _ in
            relocationCalls += 1
            return []
        }
        let request = BlockMoveRequest(
            moveId: moveId.uuidString.lowercased(),
            sourceSlug: "2026-07-13",
            rootBid: "61616161-6161-6161-6161-616161616161",
            destinationSlug: "2026-07-12"
        )

        let moveTask = service.enqueueBackendMutation { reservation in
            await gate.wait()
            try await service.moveSubtree(request, reservation: reservation)
        }
        let moveIsBlocked = await gate.waitUntilBlocked()
        XCTAssertTrue(moveIsBlocked)
        service.attach(backend: .relay)
        gate.release()

        do {
            try await moveTask.value
            XCTFail("expected the stale move lease to be rejected")
        } catch is CancellationError {
            // Expected.
        } catch {
            XCTFail("unexpected error: \(error)")
        }
        XCTAssertEqual(relocationCalls, 0)
    }

    func testPendingHubMoveBlocksRelayModeMove() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let outboxURL = directory.appendingPathComponent("relocation-outbox.json")
        let store = RelocationOutboxStore(url: outboxURL)
        let request = BlockMoveRequest(
            moveId: moveId.uuidString.lowercased(),
            sourceSlug: "2026-07-13",
            rootBid: "61616161-6161-6161-6161-616161616161",
            destinationSlug: "2026-07-12"
        )
        try store.save(PendingRelocationDelivery(
            hubIdentity: "http://mac.local:7474|legacy|/mosaics/work",
            request: request,
            prepared: nil
        ))
        let ticker = RelayTicker(relocationOutboxURL: outboxURL)
        ticker.hubMode = false

        do {
            _ = try await ticker.moveSubtreeAndDeliver(request)
            XCTFail("expected the saved hub move to block a relay-mode move")
        } catch {
            XCTAssertTrue(ticker.hasPendingRelocation)
        }
    }

    func testPendingRelocationStateSurvivesFailedDurableRemoval() throws {
        enum Expected: Error { case removalFailed }
        let delivery = PendingRelocationDelivery(
            hubIdentity: "http://mac.local:7474|legacy|",
            request: BlockMoveRequest(
                moveId: moveId.uuidString.lowercased(),
                sourceSlug: "2026-07-13",
                rootBid: "61616161-6161-6161-6161-616161616161",
                destinationSlug: "2026-07-12"
            ),
            prepared: nil
        )
        var pending: PendingRelocationDelivery? = delivery

        XCTAssertThrowsError(
            try RelayTicker.clearPendingRelocationState(&pending) {
                throw Expected.removalFailed
            }
        )
        XCTAssertEqual(pending, delivery)

        try RelayTicker.clearPendingRelocationState(&pending) {}
        XCTAssertNil(pending)
    }

    func testRejectedAndConflictedRelocationsAreTerminalDuringRetry() {
        XCTAssertTrue(RelayTicker.isTerminalRelocationError(
            FfiSyncError.RelocationRejected(message: "rejected")
        ))
        XCTAssertTrue(RelayTicker.isTerminalRelocationError(
            FfiSyncError.RelocationConflict(message: "conflict")
        ))
        XCTAssertFalse(RelayTicker.isTerminalRelocationError(
            FfiSyncError.RelocationRecoveryRequired(
                moveId: moveId.uuidString.lowercased(),
                message: "retry"
            )
        ))
        XCTAssertFalse(RelayTicker.isTerminalRelocationError(
            FfiSyncError.Other(message: "transient")
        ))
    }

    func testBackgroundCatchupRequiresRelayPairingAndUsesGroupScope() {
        func pairing(relayURL: String?) -> PairingCodeRecord {
            PairingCodeRecord(
                groupIdHex: "AABBCCDDEEFF00112233445566778899",
                groupKeyHex: String(repeating: "11", count: 32),
                deviceIdHex: String(repeating: "22", count: 16),
                url: "http://mac.local:7474",
                displayName: "Mac",
                version: 2,
                relayUrl: relayURL
            )
        }

        XCTAssertNil(RelayTicker.backgroundEngineScope(pairing: pairing(relayURL: nil)))
        XCTAssertNil(RelayTicker.backgroundEngineScope(pairing: pairing(relayURL: "  ")))
        XCTAssertEqual(
            RelayTicker.backgroundEngineScope(
                pairing: pairing(relayURL: "https://relay.example")
            ),
            MosaicEngineScope(groupIdHex: "aabbccddeeff00112233445566778899")
        )
    }

    func testSuspendedHubTemporarilyEnablesRelayAndRestoresBeforeForeground() async {
        let ticker = RelayTicker(
            relocationOutboxURL: FileManager.default.temporaryDirectory
                .appendingPathComponent(UUID().uuidString)
        )
        ticker.hubMode = true
        ticker.configureLiveHub(identity: "hub-A")

        ticker.suspendForBackground()
        XCTAssertFalse(ticker.hubMode)
        ticker.suspendForBackground()
        XCTAssertFalse(ticker.hubMode)

        await ticker.resumeFromBackground()
        XCTAssertTrue(ticker.hubMode)
    }

    func testLegacyEngineStoreIsNeverAutomaticallyAdopted() {
        XCTAssertFalse(RelayTicker.automaticallyAdoptsLegacyEngineStore)
    }

    func testScopedStoragePreparationLeavesLegacyEngineStoreUntouched() async throws {
        let documents = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: documents) }
        let legacyRoot = MosaicEngineScope.legacy.rootURL(documentsURL: documents)
        let sentinel = legacyRoot.appendingPathComponent("legacy-sentinel")
        let sentinelBytes = Data("unscoped-history".utf8)
        try FileManager.default.createDirectory(at: legacyRoot, withIntermediateDirectories: true)
        try sentinelBytes.write(to: sentinel)

        let scope = MosaicEngineScope(groupIdHex: "aabbccddeeff00112233445566778899")
        let scopedRoot = scope.rootURL(documentsURL: documents)
        XCTAssertFalse(FileManager.default.fileExists(atPath: scopedRoot.path))

        try await RelayTicker.prepareStorage(for: scope, documentsURL: documents)

        var isDirectory: ObjCBool = false
        XCTAssertTrue(FileManager.default.fileExists(atPath: scopedRoot.path, isDirectory: &isDirectory))
        XCTAssertTrue(isDirectory.boolValue)
        XCTAssertEqual(try Data(contentsOf: sentinel), sentinelBytes)
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: scopedRoot.path), [])
    }

    func testBackgroundHubLeaseDoesNotOverrideAProfileActivation() async {
        let ticker = RelayTicker(
            relocationOutboxURL: FileManager.default.temporaryDirectory
                .appendingPathComponent(UUID().uuidString)
        )
        ticker.hubMode = true
        ticker.configureLiveHub(identity: "hub-A")
        ticker.suspendForBackground()
        XCTAssertFalse(ticker.hubMode)

        ticker.configureLiveHub(identity: nil)
        await ticker.resumeFromBackground()

        XCTAssertFalse(ticker.hubMode)
    }

    func testRelayOperationAdmissionQueuesFlushBehindRelocationAndExcludesOtherDrivers() async {
        let admission = RelayOperationAdmission()
        let gate = ActivationTestGate()
        guard let relocationLease = admission.tryAcquire(.relocation) else {
            return XCTFail("relocation should acquire relay admission")
        }

        let held = Task { @MainActor in
            await gate.wait()
            admission.release(relocationLease)
        }
        let didBlock = await gate.waitUntilBlocked()
        XCTAssertTrue(didBlock)
        XCTAssertTrue(admission.isActive(relocationLease))
        XCTAssertTrue(admission.permits(relocationLease))
        XCTAssertFalse(admission.permits(nil))

        var observedIdle = false
        let idleWaiter = Task { @MainActor in
            await admission.waitUntilIdle()
            observedIdle = true
        }
        var queuedFlushAcquired = false
        let queuedFlush = Task { @MainActor in
            let lease = await admission.acquireWhenIdle(.flush)
            queuedFlushAcquired = lease != nil
            return lease
        }
        await Task.yield()
        XCTAssertFalse(observedIdle)
        XCTAssertFalse(queuedFlushAcquired)

        XCTAssertNil(admission.tryAcquire(.backgroundCatchup))
        XCTAssertNil(admission.tryAcquire(.flush))
        XCTAssertNil(admission.tryAcquire(.tick))
        XCTAssertNil(admission.tryAcquire(.immediateOutbound))
        XCTAssertNil(admission.tryAcquire(.relocation))

        gate.release()
        await held.value
        XCTAssertFalse(admission.isActive(relocationLease))
        let flushLease = await queuedFlush.value
        await idleWaiter.value
        XCTAssertTrue(observedIdle)
        XCTAssertTrue(queuedFlushAcquired)
        XCTAssertFalse(admission.permits(nil))
        guard let flushLease else {
            return XCTFail("queued flush should acquire after the active operation releases")
        }
        XCTAssertEqual(flushLease.kind, .flush)
        admission.release(flushLease)
        XCTAssertTrue(admission.permits(nil))
    }

    func testEngineOperationAdmissionDrainsExistingAndQueuesNewAcrossRelocation() async {
        let admission = EngineOperationAdmission()
        XCTAssertTrue(admission.tryBeginOperation())
        guard let relocationLease = admission.closeForExclusiveAccess() else {
            return XCTFail("relocation should close ordinary engine admission")
        }

        var relocationReady = false
        let relocation = Task { @MainActor in
            await admission.waitUntilExclusiveAccessReady(relocationLease)
            relocationReady = true
        }
        var queuedEditStarted = false
        let queuedEdit = Task { @MainActor in
            let admitted = await admission.beginOperationWhenAvailable()
            queuedEditStarted = admitted
        }
        await Task.yield()
        XCTAssertFalse(relocationReady)
        XCTAssertFalse(queuedEditStarted)

        admission.finishOperation()
        await relocation.value
        XCTAssertTrue(relocationReady)
        XCTAssertFalse(queuedEditStarted)
        XCTAssertEqual(admission.activeCount, 0)

        admission.finishExclusiveAccess(relocationLease)
        XCTAssertEqual(
            admission.activeCount,
            1,
            "the queued final edit must own a reservation before activation can race it"
        )
        await queuedEdit.value
        XCTAssertTrue(queuedEditStarted)
        admission.finishOperation()
        XCTAssertEqual(admission.activeCount, 0)
    }

    func testEngineOperationAdmissionCountsSynchronousQueueBeforeTaskStarts() async {
        let admission = EngineOperationAdmission()
        guard let relocationLease = admission.closeForExclusiveAccess() else {
            return XCTFail("relocation should close ordinary engine admission")
        }

        XCTAssertEqual(admission.reserveOperation(), .queued)
        XCTAssertEqual(admission.activeCount, 0)
        XCTAssertEqual(
            admission.operationCount,
            1,
            "a synchronous final edit must block activation before its task is scheduled"
        )

        admission.finishExclusiveAccess(relocationLease)
        XCTAssertEqual(admission.activeCount, 0)
        XCTAssertEqual(admission.operationCount, 1)
        XCTAssertNil(
            admission.closeForExclusiveAccess(),
            "a later relocation must not leapfrog the synchronously reserved edit"
        )

        let reservedOperationStarted = await admission.beginReservedOperationWhenAvailable()
        XCTAssertTrue(reservedOperationStarted)
        XCTAssertEqual(admission.activeCount, 1)
        XCTAssertEqual(admission.operationCount, 1)
        admission.finishOperation()
        XCTAssertEqual(admission.operationCount, 0)
    }

    func testBackgroundRelayModeWaitsForRelocationAndExactHubOutbox() {
        XCTAssertFalse(RelayTicker.canSuspendForBackgroundRelay(
            relocationInFlight: true,
            hasPendingRelocation: false
        ))
        XCTAssertFalse(RelayTicker.canSuspendForBackgroundRelay(
            relocationInFlight: false,
            hasPendingRelocation: true
        ))
        XCTAssertTrue(RelayTicker.canSuspendForBackgroundRelay(
            relocationInFlight: false,
            hasPendingRelocation: false
        ))
    }

    func testSuspendForBackgroundKeepsHubModeWhileExactRelocationOutboxIsPending() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let outboxURL = directory.appendingPathComponent("relocation-outbox.json")
        let hubIdentity = "http://mac.local:7474|personal|/mosaics/personal|aabb"
        try RelocationOutboxStore(url: outboxURL).save(PendingRelocationDelivery(
            hubIdentity: hubIdentity,
            request: BlockMoveRequest(
                moveId: UUID().uuidString.lowercased(),
                sourceSlug: "2026-07-13",
                rootBid: "61616161-6161-6161-6161-616161616161",
                destinationSlug: "2026-07-12"
            ),
            prepared: nil,
            engineScope: MosaicEngineScope(groupIdHex: "aabb")
        ))
        let ticker = RelayTicker(relocationOutboxURL: outboxURL)
        ticker.hubMode = true

        ticker.suspendForBackground()

        XCTAssertTrue(ticker.hubMode)
        XCTAssertTrue(ticker.hasPendingRelocation)
    }

    func testForegroundResumeInvalidatesQueuedBackgroundRelayTransition() {
        XCTAssertTrue(RelayTicker.shouldCommitBackgroundTransition(
            issuedGeneration: 4,
            currentGeneration: 4
        ))
        XCTAssertFalse(RelayTicker.shouldCommitBackgroundTransition(
            issuedGeneration: 4,
            currentGeneration: 5
        ))
    }

    func testRelayTickerRoutesEveryCoordinatorDriverThroughExclusiveAdmission() throws {
        let projectRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: projectRoot.appendingPathComponent("Sources/Data/RelayTicker.swift"),
            encoding: .utf8
        )

        func section(from start: String, to end: String) throws -> String {
            let startRange = try XCTUnwrap(source.range(of: start))
            let tail = source[startRange.lowerBound...]
            let endRange = try XCTUnwrap(tail.range(of: end))
            return String(tail[..<endRange.lowerBound])
        }

        XCTAssertTrue(source.contains(
            "private let relayOperationAdmission = RelayOperationAdmission()"
        ))
        let flushSection = try section(
            from: "func flushPendingOutbound()",
            to: "/// One-shot relay catch-up"
        )
        XCTAssertTrue(flushSection.contains("acquireWhenIdle(.flush)"))
        XCTAssertTrue(flushSection.contains("maybeRegisterApnsToken"))
        let backgroundFlushSection = try section(
            from: "func flushOnBackground()",
            to: "private func runLoop()"
        )
        XCTAssertTrue(backgroundFlushSection.contains("beginBackgroundFlush()"))
        let relocationWait = try XCTUnwrap(
            backgroundFlushSection.range(of: "waitUntilRelocationDeliveryFinishes()")
        )
        let suspension = try XCTUnwrap(
            backgroundFlushSection.range(of: "suspendForBackground()")
        )
        XCTAssertLessThan(
            backgroundFlushSection.distance(
                from: backgroundFlushSection.startIndex,
                to: relocationWait.lowerBound
            ),
            backgroundFlushSection.distance(
                from: backgroundFlushSection.startIndex,
                to: suspension.lowerBound
            )
        )
        XCTAssertTrue(backgroundFlushSection.contains("backgroundTransitionGeneration"))
        let resumeSection = try section(
            from: "func resumeFromBackground()",
            to: "/// Drain the outbound queue"
        )
        XCTAssertTrue(resumeSection.contains("waitUntilBackgroundFlushesFinish()"))
        let invalidateTransition = try XCTUnwrap(
            resumeSection.range(of: "backgroundTransitionGeneration &+=")
        )
        let restoreLease = try XCTUnwrap(
            resumeSection.range(of: "guard let lease = suspendedHubLease")
        )
        XCTAssertLessThan(
            resumeSection.distance(
                from: resumeSection.startIndex,
                to: invalidateTransition.lowerBound
            ),
            resumeSection.distance(from: resumeSection.startIndex, to: restoreLease.lowerBound)
        )
        let tickSection = try section(
            from: "private func tickOnce()",
            to: "/// The actual ensure-coordinator"
        )
        let tickAdmission = try XCTUnwrap(tickSection.range(of: "tryAcquire(.tick)"))
        let tickGeneration = try XCTUnwrap(tickSection.range(of: "tickGeneration &+="))
        XCTAssertLessThan(
            tickSection.distance(from: tickSection.startIndex, to: tickAdmission.lowerBound),
            tickSection.distance(from: tickSection.startIndex, to: tickGeneration.lowerBound)
        )
        XCTAssertTrue(tickSection.contains("relayOperationLease: relayLease"))
        let timeoutSection = try section(
            from: "guard !finishedInTime else { return }",
            to: "lastError = \"sync tick exceeded"
        )
        XCTAssertTrue(timeoutSection.contains("isActive(relayLease)"))
        XCTAssertTrue(timeoutSection.contains("tickGeneration &+="))
        XCTAssertTrue(try section(
            from: "func runBackgroundCatchup()",
            to: "static func backgroundEngineScope"
        ).contains("tryAcquire(.backgroundCatchup)"))
        XCTAssertTrue(try section(
            from: "func moveSubtreeAndDeliver(",
            to: "@discardableResult"
        ).contains("waitUntilExclusiveAccessReady"))
        XCTAssertTrue(try section(
            from: "func retryPendingRelocation()",
            to: "var hasPendingRelocation"
        ).contains("waitUntilExclusiveAccessReady"))
        XCTAssertTrue(try section(
            from: "func enqueueRecordAndPush(",
            to: "private func recordAndPushUnderLease"
        ).contains("enqueueEngineOperation"))
        XCTAssertTrue(try section(
            from: "func enqueueSpliceAndPush(",
            to: "private func spliceAndPushUnderLease"
        ).contains("enqueueEngineOperation"))
        XCTAssertTrue(try section(
            from: "func setBlockPropertyAndPush(",
            to: "// ─── Saved views registry"
        ).contains("beginEngineOperationWhenAvailable"))
        XCTAssertTrue(try section(
            from: "func applyInboundDelta(",
            to: "/// Produce the live"
        ).contains("beginEngineOperationWhenAvailable"))
        XCTAssertGreaterThanOrEqual(
            source.components(separatedBy: "tryAcquireImmediateOutbound()").count - 1,
            5
        )
    }

    func testBackgroundCatchupOutcomeMapsTruthfullyToAPNsResult() {
        XCTAssertEqual(
            AppDelegate.fetchResult(for: .completed(newData: true)),
            .newData
        )
        XCTAssertEqual(
            AppDelegate.fetchResult(for: .completed(newData: false)),
            .noData
        )
        XCTAssertEqual(AppDelegate.fetchResult(for: .unavailable), .noData)
        XCTAssertEqual(AppDelegate.fetchResult(for: .failed("offline")), .failed)
        XCTAssertTrue(BackgroundCatchupOutcome.completed(newData: false).didRunSuccessfully)
        XCTAssertFalse(BackgroundCatchupOutcome.unavailable.didRunSuccessfully)
    }
}
