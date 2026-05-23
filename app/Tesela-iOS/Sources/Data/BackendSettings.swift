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
