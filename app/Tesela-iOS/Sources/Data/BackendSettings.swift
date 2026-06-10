import SwiftUI

/// Where the iOS app reads its mosaic data from. Persisted via
/// `@AppStorage` so the choice survives relaunch.
@MainActor
final class BackendSettings: ObservableObject {
    @AppStorage("backend.mode") private var modeRaw: String = "mock"
    @AppStorage("backend.serverURL") var serverURL: String = "http://127.0.0.1:7474"
    @AppStorage("bareDateField") var bareDateField: String = "scheduled"

    enum Mode: String {
        case mock = "mock"
        case http = "http"
        /// Local-first relay mode: the UI reads the on-device engine's
        /// relay-synced materialized notes (no Mac HTTP), while the RelayTicker
        /// syncs in the background. This is the "sync with the Mac off" path —
        /// distinct from `.http` (Mac-direct, gates the relay off) and `.mock`
        /// (built-in fake snapshot). Set by pairing to a relay-only node.
        case relay = "relay"
    }

    var mode: Mode {
        get { Mode(rawValue: modeRaw) ?? .mock }
        set {
            modeRaw = newValue.rawValue
            objectWillChange.send()
        }
    }

    /// Resolves the current settings into a concrete `Mosaic.Backend`
    /// value. Returns `.mock` when the URL is unparseable so the app
    /// never falls into a broken state.
    var backend: MockMosaicService.Backend {
        Self.resolveBackend(mode: mode, serverURL: serverURL)
    }

    /// Pure mode→backend mapping (extracted for unit testing, audit A13).
    /// `.relay` ignores the server URL entirely — the relay tick's pairing
    /// code carries the relay identity; there is no Mac HTTP to point at.
    static func resolveBackend(mode: Mode, serverURL: String) -> MockMosaicService.Backend {
        if mode == .relay {
            return .relay
        }
        guard mode == .http,
              let trimmed = serverURL.trimmingCharacters(in: .whitespaces).removingPercentEncoding,
              !trimmed.isEmpty,
              let url = URL(string: trimmed),
              url.scheme != nil,
              url.host != nil
        else {
            return .mock
        }
        return .http(url)
    }
}
