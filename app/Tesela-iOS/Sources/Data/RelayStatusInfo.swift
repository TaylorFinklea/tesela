import Foundation

/// Mirrors the Rust `RelayStatus` struct returned by
/// `GET /sync/relay/status` on the Mac's tesela-server. Fields are
/// snake_case for `Codable` interop with the JSON wire format.
///
/// `configured == false` means the Mac has no `[sync.relay]` block in
/// its `config.toml` — iOS surfaces this as "your Mac isn't paired
/// with a relay yet" rather than as an error.
struct RelayStatusInfo: Codable, Equatable {
    let configured: Bool
    let url: String?
    let inbound_cursor: Int64
    let outbound_cursor_ntp: Int64?
    /// Unix seconds. `nil` when nothing has been polled yet.
    let last_poll_at: Int64?
    /// Unix seconds. `nil` when nothing has been PUT yet.
    let last_put_at: Int64?
    /// Unix seconds. `nil` when registration hasn't happened (relay
    /// unreachable / hijack-detected / first boot).
    let registered_at: Int64?
    /// Most recent error string from the relay daemon's last tick;
    /// cleared on next successful tick. `nil` when healthy.
    let last_error: String?
}
