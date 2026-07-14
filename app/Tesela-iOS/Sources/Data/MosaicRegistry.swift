import Foundation
import Combine

/// Device-local store of `MosaicProfile`s plus which one is currently
/// active. Persists to UserDefaults as JSON so the list survives
/// relaunch. When the active profile changes, observers can read
/// `activeProfile?.serverURL` to know which backend to attach to.
final class MosaicRegistry: ObservableObject {
    @Published private(set) var profiles: [MosaicProfile] = []
    @Published private(set) var activeID: UUID? = nil

    /// Synchronous pre-publication barrier for the shell's backend admission.
    /// The callback runs before `activeID` (or the active profile's routing
    /// fields) change, so a suspended activation cannot resume and publish the
    /// old profile under the new UI selection during SwiftUI's later
    /// `.onChange` turn.
    var willChangeActiveProfile: (() -> Void)?

    private let profilesKey = "mosaics.profiles.v1"
    private let activeKey = "mosaics.activeID.v1"

    var activeProfile: MosaicProfile? {
        guard let id = activeID else { return nil }
        return profiles.first(where: { $0.id == id })
    }

    init() {
        load()
    }

    func add(_ profile: MosaicProfile, makeActive: Bool = true) {
        profiles.append(profile)
        if makeActive {
            publishActiveID(profile.id)
        }
        persist()
    }

    func update(_ profile: MosaicProfile) {
        guard let idx = profiles.firstIndex(where: { $0.id == profile.id }) else { return }
        if activeID == profile.id, profiles[idx] != profile {
            willChangeActiveProfile?()
        }
        profiles[idx] = profile
        persist()
    }

    func delete(_ id: UUID) {
        let deletingActive = activeID == id
        if deletingActive {
            willChangeActiveProfile?()
        }
        profiles.removeAll { $0.id == id }
        if deletingActive {
            activeID = profiles.first?.id
        }
        persist()
    }

    func setActive(_ id: UUID) {
        guard profiles.contains(where: { $0.id == id }) else { return }
        publishActiveID(id)
        persist()
    }

    /// Migration helper: if no profiles exist yet but a legacy
    /// `backend.serverURL` is set, seed the first profile from it
    /// so existing users don't lose their connection.
    func seedFromLegacyIfNeeded(legacyURL: String, defaultName: String = "My mosaic") {
        guard profiles.isEmpty else { return }
        let trimmed = legacyURL.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }
        let seeded = MosaicProfile(name: defaultName, serverURL: trimmed)
        add(seeded, makeActive: true)
    }

    // MARK: - Discovery

    /// Pull the mosaics a server knows about and add any not already in
    /// the registry, keyed by `serverURL` + `mosaicPath`. Best-effort —
    /// silently no-ops if the server is unreachable, so callers (pairing,
    /// add-mosaic) keep working even when discovery fails.
    ///
    /// `activateCurrent` makes the server's currently-served mosaic the
    /// active profile — used by the pairing handoff so joining a server
    /// lands you on whatever it is serving.
    @MainActor
    func importDiscovered(serverURL: String, activateCurrent: Bool = false) async {
        let list: [MosaicServerClient.DiscoveredMosaic]
        do {
            list = try await MosaicServerClient.discovered(serverURL: serverURL)
        } catch {
            return
        }
        for m in list where !profiles.contains(where: {
            $0.serverURL == serverURL && $0.mosaicPath == m.path
        }) {
            profiles.append(MosaicProfile(name: m.name, serverURL: serverURL, mosaicPath: m.path))
        }
        var currentID: UUID?
        if let current = list.first(where: { $0.is_current }) {
            currentID = profiles.first {
                $0.serverURL == serverURL && $0.mosaicPath == current.path
            }?.id
        }
        if activateCurrent, let currentID {
            publishActiveID(currentID)
        } else if activeID == nil {
            publishActiveID(currentID ?? profiles.first?.id)
        }
        persist()
    }

    private func publishActiveID(_ id: UUID?) {
        guard activeID != id else { return }
        willChangeActiveProfile?()
        activeID = id
    }

    // MARK: - Persistence

    private func load() {
        if let data = UserDefaults.standard.data(forKey: profilesKey),
           let list = try? JSONDecoder().decode([MosaicProfile].self, from: data) {
            profiles = list
        }
        if let raw = UserDefaults.standard.string(forKey: activeKey),
           let id = UUID(uuidString: raw) {
            activeID = id
        }
        // Clean up dangling activeID if the profile was deleted out-of-band.
        if let id = activeID, !profiles.contains(where: { $0.id == id }) {
            activeID = profiles.first?.id
        }
    }

    private func persist() {
        if let data = try? JSONEncoder().encode(profiles) {
            UserDefaults.standard.set(data, forKey: profilesKey)
        }
        if let id = activeID {
            UserDefaults.standard.set(id.uuidString, forKey: activeKey)
        } else {
            UserDefaults.standard.removeObject(forKey: activeKey)
        }
    }
}

// MARK: - Server client

/// Stateless HTTP client for a `tesela-server`'s mosaic-management
/// endpoints. Kept separate from `MockMosaicService` (which serves one
/// mosaic's data) because these calls can target a server that isn't
/// the active backend yet — e.g. while adding a mosaic or pairing.
enum MosaicServerClient {

    /// One mosaic a server knows about on disk. Mirrors the server's
    /// `DiscoveredMosaic` JSON.
    struct DiscoveredMosaic: Decodable, Identifiable, Hashable {
        let name: String
        let path: String
        let is_current: Bool
        let note_count: Int
        let last_modified: String?
        var id: String { path }
    }

    struct CurrentIdentity: Decodable, Equatable {
        let path: String
        let groupIdHex: String

        enum CodingKeys: String, CodingKey {
            case path
            case groupIdHex = "group_id_hex"
        }
    }

    enum ClientError: LocalizedError {
        case badURL
        case http(Int, String)
        case mosaicSwitchNotConfirmed(expected: String, observed: String?)

        var errorDescription: String? {
            switch self {
            case .badURL:
                return "That doesn't look like a valid server URL."
            case let .http(code, body):
                return "Server returned HTTP \(code)" + (body.isEmpty ? "" : ": \(body)")
            case let .mosaicSwitchNotConfirmed(expected, observed):
                let actual = observed.map { " (still serving \($0))" } ?? ""
                return "Server did not restart on mosaic \(expected)\(actual)."
            }
        }
    }

    /// GET /mosaics/discovered — every mosaic the server can see on disk.
    static func discovered(serverURL: String) async throws -> [DiscoveredMosaic] {
        try await get(serverURL, "/mosaics/discovered")
    }

    /// GET /mosaics/current — path of the mosaic the server is serving.
    static func currentPath(serverURL: String) async throws -> String {
        struct Resp: Decodable { let path: String }
        let resp: Resp = try await get(serverURL, "/mosaics/current")
        return resp.path
    }

    /// One atomic observation of both the served path and its physical sync
    /// group. Reading these from separate endpoints admits path A -> group B
    /// -> path A during a fast server restart (an ABA identity mix-up).
    static func currentIdentity(serverURL: String) async throws -> CurrentIdentity {
        let identity: CurrentIdentity = try await get(serverURL, "/mosaics/current")
        return CurrentIdentity(
            path: identity.path,
            groupIdHex: identity.groupIdHex.lowercased()
        )
    }

    /// Stable physical identity of the mosaic currently served at this URL.
    /// Unlike URL/path or a device-local profile UUID, the sync group id is
    /// identical when the same mosaic is reached later through its relay.
    static func currentGroupIdHex(serverURL: String) async throws -> String {
        struct Resp: Decodable { let code: String }
        let response: Resp = try await get(serverURL, "/sync/peer/pairing-code")
        return try decodePairingCode(code: response.code).groupIdHex
    }

    /// POST /mosaics/switch — persist a new default mosaic. Takes effect
    /// only after `restart`.
    static func switchMosaic(serverURL: String, path: String) async throws -> String {
        struct Resp: Decodable {
            let defaultMosaic: String

            enum CodingKeys: String, CodingKey {
                case defaultMosaic = "default_mosaic"
            }
        }
        let resp: Resp = try await postDecoding(
            serverURL,
            "/mosaics/switch",
            body: ["path": path]
        )
        return resp.defaultMosaic
    }

    /// POST /server/restart — graceful restart so a switched mosaic takes
    /// effect. Errors remain observable; in particular, embedded-server 409
    /// must never be mistaken for a successful switch.
    static func restart(serverURL: String) async throws {
        try await post(serverURL, "/server/restart", body: [:])
    }

    /// POST /mosaics — create a new named mosaic; returns its on-disk path.
    static func createMosaic(serverURL: String, name: String) async throws -> String {
        struct Resp: Decodable { let path: String }
        let resp: Resp = try await postDecoding(serverURL, "/mosaics", body: ["name": name])
        return resp.path
    }

    // MARK: Plumbing

    private static func url(_ serverURL: String, _ path: String) throws -> URL {
        let base = serverURL.trimmingCharacters(in: .whitespaces)
        let trimmed = base.hasSuffix("/") ? String(base.dropLast()) : base
        guard let u = URL(string: trimmed + path), u.scheme != nil, u.host != nil else {
            throw ClientError.badURL
        }
        return u
    }

    private static func get<T: Decodable>(_ serverURL: String, _ path: String) async throws -> T {
        var req = URLRequest(url: try url(serverURL, path))
        req.timeoutInterval = 8
        req.cachePolicy = .reloadIgnoringLocalCacheData
        let (data, resp) = try await URLSession.shared.data(for: req)
        try ensureOK(resp, data)
        return try JSONDecoder().decode(T.self, from: data)
    }

    private static func post(_ serverURL: String, _ path: String, body: [String: String]) async throws {
        let (data, resp) = try await rawPost(serverURL, path, body: body)
        try ensureOK(resp, data)
    }

    private static func postDecoding<T: Decodable>(
        _ serverURL: String, _ path: String, body: [String: String]
    ) async throws -> T {
        let (data, resp) = try await rawPost(serverURL, path, body: body)
        try ensureOK(resp, data)
        return try JSONDecoder().decode(T.self, from: data)
    }

    private static func rawPost(
        _ serverURL: String, _ path: String, body: [String: String]
    ) async throws -> (Data, URLResponse) {
        var req = URLRequest(url: try url(serverURL, path))
        req.httpMethod = "POST"
        req.timeoutInterval = 20
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONSerialization.data(withJSONObject: body)
        return try await URLSession.shared.data(for: req)
    }

    private static func ensureOK(_ resp: URLResponse, _ data: Data) throws {
        guard let http = resp as? HTTPURLResponse else { return }
        guard (200..<300).contains(http.statusCode) else {
            throw ClientError.http(
                http.statusCode,
                String(data: data.prefix(160), encoding: .utf8) ?? ""
            )
        }
    }
}
