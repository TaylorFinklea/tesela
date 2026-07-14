import Foundation
import CryptoKit

/// Stable local-engine namespace for one physical mosaic. Profile UUID and
/// transport are deliberately absent: the same mosaic reached over a live
/// server or its relay must share one CRDT store, while different group
/// identities must never share files, version baselines, or engine history.
enum MosaicEngineScope: Codable, Equatable, Hashable, Sendable {
    case legacy
    case mosaic(groupIdHex: String)

    init(groupIdHex: String) {
        self = .mosaic(
            groupIdHex: groupIdHex
                .trimmingCharacters(in: .whitespacesAndNewlines)
                .lowercased()
        )
    }

    var identity: String {
        switch self {
        case .legacy:
            return "legacy"
        case .mosaic(let groupIdHex):
            return "group|\(groupIdHex)"
        }
    }

    var storageKey: String {
        let digest = SHA256.hash(data: Data(identity.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    func rootURL(documentsURL: URL? = nil) -> URL {
        let documents = documentsURL ?? FileManager.default.urls(
            for: .documentDirectory,
            in: .userDomainMask
        )[0]
        switch self {
        case .legacy:
            return documents.appendingPathComponent("sync-ios-mosaic")
        case .mosaic:
            return documents
                .appendingPathComponent("sync-ios-mosaics", isDirectory: true)
                .appendingPathComponent(storageKey, isDirectory: true)
        }
    }
}

/// A named mosaic the user can switch between on this device. The
/// active mosaic is the one whose `MockMosaicService` is currently
/// rendered on screen; switching profiles re-attaches the backend
/// to the new profile's server URL.
///
/// This is device-local: a profile list isn't synced across devices.
/// Your phone's "Personal" being active has no effect on what your
/// desktop has open. (Each device picks its own active mosaic, like
/// Obsidian vaults.)
struct MosaicProfile: Identifiable, Codable, Equatable, Hashable {
    let id: UUID
    var name: String
    var serverURL: String
    var authToken: String?
    /// SF Symbol shown in the TopBar slot (replaces the old sync dot).
    /// Color is driven by reachability, not by the profile.
    var iconSymbol: String
    /// On-disk path of this mosaic on its server. Multiple profiles can
    /// share one `serverURL` and differ only by `mosaicPath`; switching
    /// to such a profile asks the server to switch+restart onto it.
    /// `nil` = a legacy profile that only knows a URL (the server's
    /// current mosaic, whatever that is).
    var mosaicPath: String?

    init(
        id: UUID = UUID(),
        name: String,
        serverURL: String,
        authToken: String? = nil,
        iconSymbol: String = "circle.grid.3x3",
        mosaicPath: String? = nil
    ) {
        self.id = id
        self.name = name
        self.serverURL = serverURL
        self.authToken = authToken
        self.iconSymbol = iconSymbol
        self.mosaicPath = mosaicPath
    }
}

/// True when `urlString`'s host is a loopback address — `127.0.0.1`,
/// `localhost`, or `::1`. A loopback URL only reaches a server on the
/// *same device*: from a physical iPhone it can never reach a Mac.
/// (It does work from the iOS simulator, which shares the host Mac's
/// network namespace — the historical reason this slipped past QA.)
func isLoopbackURL(_ urlString: String) -> Bool {
    let trimmed = urlString.trimmingCharacters(in: .whitespaces)
    guard let host = URLComponents(string: trimmed)?.host?.lowercased() else {
        return false
    }
    return host == "127.0.0.1" || host == "localhost" || host == "::1"
}

/// Curated palette for the icon picker. Tapping "Other…" in the picker
/// lets the user paste any SF Symbol name, but most users pick one of
/// these. Kept small + recognizable on purpose.
let mosaicIconPalette: [String] = [
    "circle.grid.3x3",
    "house",
    "briefcase",
    "person.circle",
    "leaf",
    "book",
    "graduationcap",
    "hammer",
    "paintbrush",
    "lightbulb",
    "sparkles",
    "brain.head.profile",
    "code",
    "gamecontroller",
    "music.note",
    "heart",
]
