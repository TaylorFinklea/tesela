import Foundation
import SwiftUI

/// Holds the latest caret of each OTHER peer (Phase 3 iOS presence), keyed by
/// peer id, with a timeout so a peer that goes quiet (or disconnects) fades.
/// `LiveSyncSocket.onPresence` feeds it decoded frames; the editor reads
/// `cursors(forSlug:bid:)` to draw the carets in each block. Mirrors the web
/// `remote-cursors.ts`.
@MainActor
final class RemoteCursorStore: ObservableObject {
    /// A peer that hasn't refreshed within this window is treated as gone.
    private let staleInterval: TimeInterval = 10

    /// peer id → latest frame. `@Published` so SwiftUI overlays re-render.
    @Published private(set) var byPeer: [String: LoroPresence.Frame] = [:]
    private var lastSeen: [String: Date] = [:]

    /// This launch's stable peer id + its deterministic color.
    let localPeer: String = UUID().uuidString
    lazy var localColor: String = Self.color(for: localPeer)

    private static let palette = [
        "#ef4444", "#f97316", "#eab308", "#22c55e",
        "#06b6d4", "#3b82f6", "#a855f7", "#ec4899",
    ]

    /// Parse a `#RRGGBB` presence color string to a SwiftUI `Color` (falls back
    /// to blue). Bridges the wire format to the existing `Color(hex: UInt32)`.
    static func displayColor(_ hex: String) -> Color {
        let s = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        return Color(hex: UInt32(s, radix: 16) ?? 0x3b82f6)
    }

    /// Deterministic palette color for a peer id (FNV-1a, mirrors the web).
    static func color(for peer: String) -> String {
        var h: UInt32 = 2166136261
        for b in peer.utf8 {
            h ^= UInt32(b)
            h = h &* 16777619
        }
        return palette[Int(h % UInt32(palette.count))]
    }

    /// Merge a peer's presence frame. Our OWN frames are ignored.
    func apply(_ frame: LoroPresence.Frame, now: Date = Date()) {
        guard frame.peer != localPeer else { return }
        byPeer[frame.peer] = frame
        lastSeen[frame.peer] = now
    }

    /// The live (non-stale) remote cursors that fall in a given note + block.
    /// Sorted by `peer` id so the order is STABLE across re-renders —
    /// `Dictionary.values` has no defined order, and that order flows into the
    /// block's presence-chip cluster (which peers show + the `+N` overflow) and
    /// keys the whole-row tint off the first caret's color, so an unsorted
    /// result reshuffles (flickers) on every inbound frame / prune / keystroke.
    func cursors(forSlug slug: String, bid: String, now: Date = Date()) -> [LoroPresence.Frame] {
        byPeer.values
            .filter { c in
                guard c.slug == slug, c.bid == bid, let seen = lastSeen[c.peer] else { return false }
                return now.timeIntervalSince(seen) <= staleInterval
            }
            .sorted { $0.peer < $1.peer }
    }

    /// Drop cursors past the staleness window. Returns whether anything changed.
    @discardableResult
    func pruneStale(now: Date = Date()) -> Bool {
        let dead = lastSeen.filter { now.timeIntervalSince($0.value) > staleInterval }.map(\.key)
        guard !dead.isEmpty else { return false }
        for peer in dead {
            byPeer.removeValue(forKey: peer)
            lastSeen.removeValue(forKey: peer)
        }
        return true
    }

    func clear() {
        byPeer = [:]
        lastSeen = [:]
    }
}
