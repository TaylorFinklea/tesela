import XCTest
@testable import Tesela

/// Saved-views surface (saved-views spec, 2026-06-10): the pure logic
/// behind the Inbox tab's view switcher — DSL validation (the iOS mirror
/// of the server's `validate_dsl` rule from routes/views.rs), fragment
/// insertion (chips as one-tap writers into the query string), selection
/// persistence/resolution, and the service's backend routing through the
/// `.relay` engine seams.
@MainActor
final class SavedViewsTests: XCTestCase {

    // MARK: - DSL validation (server's validate_dsl mirror)

    func testValidationRejectsEmpty() {
        XCTAssertNotNil(SavedViewLogic.dslValidationError(""))
        XCTAssertNotNil(SavedViewLogic.dslValidationError("   \n "))
    }

    func testValidationRejectsZeroPredicateInput() {
        // Unknown bytes / bare punctuation parse to ZERO predicates —
        // saving would silently create a match-everything view.
        XCTAssertNotNil(SavedViewLogic.dslValidationError("???"))
        // Barewords with no operator aren't predicates (Rust parity:
        // "A bareword with no operator at all isn't a valid predicate").
        XCTAssertNotNil(SavedViewLogic.dslValidationError("hello world"))
    }

    func testValidationAcceptsKeyValueFilters() {
        XCTAssertNil(SavedViewLogic.dslValidationError("status:doing"))
        XCTAssertNil(SavedViewLogic.dslValidationError("tag:project -has:scheduled"))
    }

    func testValidationAcceptsTheInboxDefault() {
        // The seeded builtin's DSL must always be saveable.
        XCTAssertNil(SavedViewLogic.dslValidationError(SavedView.fallbackInbox.dsl))
        XCTAssertNil(
            SavedViewLogic.dslValidationError("status:backlog,todo -has:scheduled -has:deadline")
        )
    }

    func testValidationCarveOuts() {
        // Server parity: a lone `kind:` selector and a sort-only ORDER BY
        // query are valid queries with an empty predicate tree.
        XCTAssertNil(SavedViewLogic.dslValidationError("kind:block"))
        XCTAssertNil(SavedViewLogic.dslValidationError("ORDER BY created"))
    }

    func testValidationOrderByCarveOutIsStructuralNotSubstring() {
        // Adversarial-review fix (2026-06-10): the carve-out keys off
        // whether an ORDER BY with at least one field ACTUALLY parsed —
        // the server rule (`parse_query(...).sort.is_some()`, views.rs) —
        // never off a substring. In `.relay` mode this check is the ONLY
        // gate, so a substring would persist match-everything views.
        //
        // The substring trap: "reorder bytes" contains "order by".
        XCTAssertNotNil(SavedViewLogic.dslValidationError("reorder bytes"))
        // ORDER BY with no sort field parses to NO sort (Rust
        // `parse_order_by` returns None on empty parts) → rejected.
        XCTAssertNotNil(SavedViewLogic.dslValidationError("order by"))
        XCTAssertNotNil(SavedViewLogic.dslValidationError("ORDER BY"))
        // "order" without "by" is just a dropped bareword.
        XCTAssertNotNil(SavedViewLogic.dslValidationError("order created"))
        // Parsed sorts pass — server parity for case, direction, and
        // multi-key shapes.
        XCTAssertNil(SavedViewLogic.dslValidationError("order by created desc"))
        XCTAssertNil(SavedViewLogic.dslValidationError("Order By status, deadline desc"))
        XCTAssertNil(SavedViewLogic.dslValidationError("status:todo ORDER BY deadline"))
    }

    // MARK: - Fragment insertion (chips write into the DSL string)

    func testToggleFragmentAppendsWhenMissing() {
        XCTAssertEqual(
            SavedViewLogic.toggleFragment("-has:status", in: "kind:block"),
            "kind:block -has:status"
        )
    }

    func testToggleFragmentRemovesWhenPresent() {
        XCTAssertEqual(
            SavedViewLogic.toggleFragment("-has:status", in: "kind:block -has:status -is:heading"),
            "kind:block -is:heading"
        )
    }

    func testToggleFragmentPreservesHandwrittenClauses() {
        // A raw clause the user typed (not owned by any chip) must
        // survive chip toggles verbatim.
        let dsl = "status:backlog,todo tag:\"To Read\""
        let toggled = SavedViewLogic.toggleFragment("-has:scheduled", in: dsl)
        XCTAssertEqual(toggled, "status:backlog,todo tag:\"To Read\" -has:scheduled")
        XCTAssertEqual(SavedViewLogic.toggleFragment("-has:scheduled", in: toggled), dsl)
    }

    func testFragmentActiveDetection() {
        XCTAssertTrue(SavedViewLogic.fragmentActive("-has:status", in: "kind:block -has:status"))
        XCTAssertFalse(SavedViewLogic.fragmentActive("-has:status", in: "kind:block has:status"))
    }

    // MARK: - Editor draft merge (tesela-ya4.7 — GrViewEditorSheet.save())

    func testApplyingDraftRoundTripsGroupByPick() {
        // Mirrors GrViewEditorSheet's "editing an existing view" path: the
        // sheet's picker sets `displayGroupBy` draft state, and `save()`
        // must land that pick on the persisted record.
        let base = SavedView(
            id: "v-board", name: "Board", dsl: "tag:project", order: 10,
            builtin: false, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        )
        let updated = SavedViewLogic.applyingDraft(
            to: base, name: "Board", dsl: "tag:project", displayMode: "kanban", displayGroupBy: "Status"
        )
        XCTAssertEqual(updated.displayGroupBy, "Status")
        XCTAssertEqual(updated.displayMode, "kanban")
    }

    func testApplyingDraftDefaultOptionClearsGroupBy() {
        // Picking "Default" in the picker is `nil` — must clear a
        // previously-set explicit override so decision 3's a → c → d
        // resolution order takes over again, not persist a stale value.
        let base = SavedView(
            id: "v-board", name: "Board", dsl: "tag:project", order: 10,
            builtin: false, displayMode: "kanban", displayGroupBy: "Status", displayShowDone: nil
        )
        let updated = SavedViewLogic.applyingDraft(
            to: base, name: "Board", dsl: "tag:project", displayMode: "kanban", displayGroupBy: nil
        )
        XCTAssertNil(updated.displayGroupBy)
    }

    func testApplyingDraftPreservesFieldsOutsideTheDraft() {
        // id/order/builtin/displayShowDone/displayTableConfig are never
        // part of the sheet's draft — a save must never clobber them, and
        // a builtin's `builtin` flag specifically must survive (builtins
        // stay editable-not-deletable).
        let base = SavedView(
            id: "builtin-inbox", name: "Views", dsl: "status:todo", order: 0,
            builtin: true, displayMode: "kanban", displayGroupBy: "Priority", displayShowDone: true,
            displayTableConfig: SavedViewTableConfig(hidden: ["Notes"], order: [], sortBy: nil, sortDir: nil)
        )
        let updated = SavedViewLogic.applyingDraft(
            to: base, name: "Views", dsl: "status:todo", displayMode: "kanban", displayGroupBy: "Status"
        )
        XCTAssertEqual(updated.id, "builtin-inbox")
        XCTAssertEqual(updated.order, 0)
        XCTAssertTrue(updated.builtin)
        XCTAssertEqual(updated.displayShowDone, true)
        XCTAssertEqual(updated.displayTableConfig?.hidden, ["Notes"])
        XCTAssertEqual(updated.displayGroupBy, "Status")
    }

    // MARK: - Selection persistence + resolution

    private let userViews = [
        SavedView(
            id: SavedView.builtinInboxId, name: "Views", dsl: "status:todo", order: 0,
            builtin: true, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        ),
        SavedView(
            id: "v-week", name: "This week", dsl: "has:scheduled", order: 10,
            builtin: false, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        ),
    ]

    func testResolveSelectionPersistedWins() {
        XCTAssertEqual(
            SavedViewLogic.resolveSelection(views: userViews, persisted: "v-week"),
            "v-week"
        )
    }

    func testResolveSelectionDefaultsToBuiltinInbox() {
        // No persisted choice (and a stale id) both land on the builtin
        // Inbox — the spec's default selection.
        XCTAssertEqual(
            SavedViewLogic.resolveSelection(views: userViews, persisted: nil),
            SavedView.builtinInboxId
        )
        XCTAssertEqual(
            SavedViewLogic.resolveSelection(views: userViews, persisted: "deleted-id"),
            SavedView.builtinInboxId
        )
    }

    func testResolveSelectionFallsBackToFirstView() {
        let noInbox = [userViews[1]]
        XCTAssertEqual(
            SavedViewLogic.resolveSelection(views: noInbox, persisted: nil),
            "v-week"
        )
    }

    func testSelectionKeyScopesPerBackend() {
        let relayKey = SavedViewLogic.selectionKey(
            scope: SavedViewLogic.selectionScope(mode: "relay", serverURL: "")
        )
        let httpA = SavedViewLogic.selectionKey(
            scope: SavedViewLogic.selectionScope(mode: "http", serverURL: "http://mac-a:7474")
        )
        let httpB = SavedViewLogic.selectionKey(
            scope: SavedViewLogic.selectionScope(mode: "http", serverURL: "http://mac-b:7474")
        )
        XCTAssertNotEqual(relayKey, httpA)
        XCTAssertNotEqual(httpA, httpB, "two Macs are two registries")
    }

    func testSortedOrdersByOrderThenId() {
        let shuffled = [
            SavedView(
                id: "b", name: "B", dsl: "tag:b", order: 10,
                builtin: false, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
            ),
            SavedView(
                id: "a", name: "A", dsl: "tag:a", order: 10,
                builtin: false, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
            ),
            SavedView(
                id: "z", name: "Z", dsl: "tag:z", order: 0,
                builtin: false, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
            ),
        ]
        XCTAssertEqual(SavedViewLogic.sorted(shuffled).map(\.id), ["z", "a", "b"])
    }

    // MARK: - Server JSON shape (tesela_sync::ViewRecord serde parity)

    func testBuiltinInboxDisplayCompatibilityNormalizesToViews() {
        let legacy = SavedView(
            id: SavedView.builtinInboxId, name: "Inbox", dsl: "status:todo", order: 0,
            builtin: true, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        )
        XCTAssertEqual(legacy.displayCompatible().name, "Views")

        let custom = SavedView(
            id: SavedView.builtinInboxId, name: "Triage", dsl: "status:todo", order: 0,
            builtin: true, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        )
        XCTAssertEqual(custom.displayCompatible().name, "Triage")
    }

    func testDecodesServerViewsJSON() throws {
        let json = """
        [
          {"id": "builtin-inbox", "name": "Views",
           "dsl": "status:backlog,todo -has:scheduled -has:deadline",
           "order": 0, "builtin": true, "display_mode": "list",
           "display_group_by": null, "display_show_done": null},
          {"id": "v-board", "name": "Board", "dsl": "tag:project",
           "order": 10, "builtin": false, "display_mode": "kanban",
           "display_group_by": "status", "display_show_done": false}
        ]
        """
        let views = try JSONDecoder().decode([SavedView].self, from: Data(json.utf8))
        XCTAssertEqual(views.count, 2)
        XCTAssertTrue(views[0].builtin)
        XCTAssertEqual(views[0].dsl, SavedView.fallbackInbox.dsl)
        XCTAssertEqual(views[1].displayMode, "kanban")
        XCTAssertEqual(views[1].displayGroupBy, "status")
        XCTAssertEqual(views[1].displayShowDone, false)
    }

    // MARK: - Service routing (the .relay seams; mock stays inert)

    func testMockFetchViewsServesFallbackInbox() async {
        let service = MockMosaicService()
        let views = await service.fetchViews()
        XCTAssertEqual(views, [SavedView.fallbackInbox])
    }

    func testRelayFetchViewsRoutesThroughSeam() async {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        service.onViewsList = { [self] in Array(userViews.reversed()) }
        let views = await service.fetchViews()
        // Seam output is re-sorted by (order, id).
        XCTAssertEqual(views.map(\.id), [SavedView.builtinInboxId, "v-week"])
    }

    func testRelayFetchViewsFallsBackWhenSeamUnwiredOrEmpty() async {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        let unwired = await service.fetchViews()
        XCTAssertEqual(unwired, [SavedView.fallbackInbox])
        service.onViewsList = { [] }
        let empty = await service.fetchViews()
        XCTAssertEqual(empty, [SavedView.fallbackInbox])
    }

    func testRelaySaveViewRoutesThroughUpsertSeam() async throws {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        var captured: SavedView?
        service.onViewsUpsert = { captured = $0 }
        let record = SavedView(
            id: "v-doing", name: "Doing", dsl: "status:doing", order: 10,
            builtin: false, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        )
        try await service.enqueueBackendMutation { reservation in
            try await service.saveView(
                record,
                isNew: true,
                reservation: reservation
            )
        }.value
        XCTAssertEqual(captured, record)
    }

    func testRelaySaveViewThrowsWhenSeamUnwired() async {
        // An unwired seam is indistinguishable from a failed write — must
        // throw, never silently succeed (the silent-no-op bug class).
        let service = MockMosaicService()
        service.attach(backend: .relay)
        let record = SavedView.fallbackInbox
        do {
            try await service.enqueueBackendMutation { reservation in
                try await service.saveView(
                    record,
                    isNew: false,
                    reservation: reservation
                )
            }.value
            XCTFail("expected a throw when no views seam is wired")
        } catch {
            // expected
        }
    }

    func testRelayDeleteViewRoutesThroughSeam() async throws {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        var captured: String?
        service.onViewsDelete = { captured = $0 }
        try await service.enqueueBackendMutation { reservation in
            try await service.deleteView(id: "v-doing", reservation: reservation)
        }.value
        XCTAssertEqual(captured, "v-doing")
    }

    func testDeleteBuiltinInboxRefusedClientSide() async {
        // Builtins are editable, never deletable — the engine and the
        // server both enforce it; the client pre-check turns a bypass
        // into a clear local error on every backend.
        let service = MockMosaicService()
        service.attach(backend: .relay)
        service.onViewsDelete = { _ in
            XCTFail("builtin delete must not reach the engine seam")
        }
        do {
            try await service.enqueueBackendMutation { reservation in
                try await service.deleteView(
                    id: SavedView.builtinInboxId,
                    reservation: reservation
                )
            }.value
            XCTFail("expected the builtin-delete throw")
        } catch {
            // expected
        }
    }

    func testRelayReorderUpsertsOnlyChangedOrders() async throws {
        let service = MockMosaicService()
        service.attach(backend: .relay)
        var upserts: [SavedView] = []
        service.onViewsUpsert = { upserts.append($0) }
        // v-week first, inbox second: inbox keeps order 10 → only v-week
        // (20 → 10 slot mismatch) and inbox (0 → 20)… compute explicitly:
        let inbox = SavedView(
            id: SavedView.builtinInboxId, name: "Views", dsl: "status:todo", order: 10,
            builtin: true, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        )
        let week = SavedView(
            id: "v-week", name: "This week", dsl: "has:scheduled", order: 20,
            builtin: false, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        )
        // New order: week first (→10), inbox second (→20). Both change.
        try await service.enqueueBackendMutation { reservation in
            try await service.reorderViews(
                [week, inbox],
                reservation: reservation
            )
        }.value
        XCTAssertEqual(upserts.map(\.id), ["v-week", SavedView.builtinInboxId])
        XCTAssertEqual(upserts.map(\.order), [10, 20])
        // Re-running the same order is a no-op (no redundant registry ops).
        upserts = []
        let reweek = SavedView(
            id: "v-week", name: "This week", dsl: "has:scheduled", order: 10,
            builtin: false, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        )
        let reinbox = SavedView(
            id: SavedView.builtinInboxId, name: "Views", dsl: "status:todo", order: 20,
            builtin: true, displayMode: "list", displayGroupBy: nil, displayShowDone: nil
        )
        try await service.enqueueBackendMutation { reservation in
            try await service.reorderViews(
                [reweek, reinbox],
                reservation: reservation
            )
        }.value
        XCTAssertTrue(upserts.isEmpty)
    }

    func testMockSaveAndDeleteStayInert() async throws {
        let service = MockMosaicService()
        service.onViewsUpsert = { _ in XCTFail("seam must not fire in mock mode") }
        service.onViewsDelete = { _ in XCTFail("seam must not fire in mock mode") }
        try await service.enqueueBackendMutation { reservation in
            try await service.saveView(
                SavedView.fallbackInbox,
                isNew: false,
                reservation: reservation
            )
            try await service.deleteView(
                id: "v-anything",
                reservation: reservation
            )
        }.value
    }

    // MARK: - FFI bridge round-trip

    func testFfiRecordBridgeRoundTrips() {
        let view = SavedView(
            id: "v-board", name: "Board", dsl: "tag:project", order: 30,
            builtin: false, displayMode: "kanban",
            displayGroupBy: "status", displayShowDone: true
        )
        XCTAssertEqual(SavedView(ffi: view.ffiRecord), view)
    }
}
