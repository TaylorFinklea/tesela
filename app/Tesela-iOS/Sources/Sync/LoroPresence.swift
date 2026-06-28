import Foundation

/// PRES — the ephemeral presence frame codec (Phase 3 iOS multi-device).
///
/// A frame is: `b"PRES"` (4 bytes) ++ utf8( JSON(Frame) ), byte-identical to the
/// web's `web/src/lib/loro/presence.ts` so iOS↔web cursors interoperate. Distinct
/// from the TLR2 Loro-delta magic, so `LiveSyncSocket.handle` routes it to the
/// remote-cursor store instead of the engine. The server forwards it opaquely on
/// the WS binary fan-out (`route_inbound_binary`); only the 4-byte magic matters
/// to the relay.
///
/// v1 carries a PLAIN utf16 `offset` (UITextView `selectedRange` semantics) —
/// the SAME field the web sends — rather than an op-anchored loro `Cursor`, so
/// the two platforms share one wire format. (The op-anchored `mint_cursor`
/// path stays available in the FFI for a future precise-collab mode.)
enum LoroPresence {
    /// b"PRES". Keep in sync with the Rust `WS_PRESENCE_MAGIC` + web `PRES_MAGIC`.
    static let magic: [UInt8] = [0x50, 0x52, 0x45, 0x53]

    /// One peer's live caret in a note. Field names match the web `PresenceFrame`.
    struct Frame: Codable, Equatable {
        /// Stable per-launch peer id (the sender).
        let peer: String
        /// Display color, `#RRGGBB`.
        let color: String
        /// Optional short label.
        var name: String?
        /// Note slug the caret is in.
        let slug: String
        /// Block id (the `<!-- bid:… -->` uuid) the caret is in.
        let bid: String
        /// utf16 caret offset within that block's text.
        let offset: Int
    }

    /// `true` if `data` carries the PRES magic (a presence frame, not a delta).
    static func isPresenceFrame(_ data: Data) -> Bool {
        data.count >= 4 && data.prefix(4).elementsEqual(magic)
    }

    /// Encode a presence frame: `PRES` ++ utf8(JSON).
    static func encode(_ frame: Frame) -> Data {
        var out = Data(magic)
        if let json = try? JSONEncoder().encode(frame) {
            out.append(json)
        }
        return out
    }

    /// Decode a presence frame. `nil` for a non-PRES frame (e.g. a TLR2 delta)
    /// or malformed JSON, so the caller can fall through to the delta path.
    static func decode(_ data: Data) -> Frame? {
        guard isPresenceFrame(data) else { return nil }
        return try? JSONDecoder().decode(Frame.self, from: Data(data.dropFirst(4)))
    }
}
