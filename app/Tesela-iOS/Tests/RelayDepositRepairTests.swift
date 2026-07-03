import XCTest
@testable import Tesela

/// tesela-c7s F1 (iOS half): exercises the exact FFI pair the snapshot-deposit
/// path forwards into — `exportAllNoteSnapshots()` → then
/// `repairBroadcastCursorsAfterSnapshot(deposited:)` with the SAME records —
/// proving the call compiles, links, forwards the export-time `versionVv`
/// verbatim, and safely NO-OPS an unbroadcast / healthy cursor (the iOS mirror
/// of the Rust `repair_leaves_a_healthy_cursor_untouched`). `RelayTicker.
/// depositSnapshotsIfDue` runs this same sequence after `putSnapshotsChunked`
/// confirms.
final class RelayDepositRepairTests: XCTestCase {

    func testDepositRepairForwardsExportTimeRecordsAndIsSafeNoOp() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("tesela-c7s-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let engine = try await SyncEngineHandle.openLoro(
            mosaicPath: root.path,
            deviceIdHex: "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1"
        )
        _ = try await engine.recordNoteUpsertBySlug(
            slug: "deposit-repair",
            title: "Deposit Repair",
            content: "- body <!-- bid:0d0d0d0d-0d0d-0d0d-0d0d-0d0d0d0d0d0d -->\n",
            createdAtMillis: 1
        )

        // What the deposit path exports + hands to the relay.
        let snapshots = try await engine.exportAllNoteSnapshots()
        XCTAssertEqual(snapshots.count, 1, "the authored note is tracked + exported")
        XCTAssertFalse(
            snapshots[0].versionVv.isEmpty,
            "each record carries its export-time version vector — the value the repair re-anchors to"
        )

        // The exact forward `depositSnapshotsIfDue` performs after a confirmed
        // deposit. A never-broadcast note has no outbound cursor, so this must
        // be a safe no-op: it must not throw, corrupt, or drop the note.
        await engine.repairBroadcastCursorsAfterSnapshot(deposited: snapshots)

        let after = try await engine.exportAllNoteSnapshots()
        XCTAssertEqual(
            after.count, 1,
            "the repair forward left the note intact (healthy/unbroadcast cursor untouched)"
        )
    }
}
