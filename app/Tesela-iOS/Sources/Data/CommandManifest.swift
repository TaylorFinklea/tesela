import Foundation

/// One entry from the command MANIFEST (tesela-cmdd.2) — data, not behavior:
/// id/verb/label/glyph/category/default shortcut+chord/surfaces/keywords/
/// args-shape, no run closures (those stay native to each client). Field
/// names are snake_case to match the checked-in JSON verbatim (the same
/// convention as `APINote`'s `note_type`/`modified_at`) and mirror the Rust
/// `CommandManifestEntry` (`crates/tesela-server/src/routes/commands.rs`)
/// field-for-field.
struct CommandManifestEntry: Decodable, Equatable, Hashable {
    let id: String
    let verb: String?
    let label: String
    let glyph: String
    let category: String
    let shortcut: String?
    let chord: [String]?
    let surfaces: [String]
    let keywords: [String]
    let takes_arg: Bool
    let arg_prompt: String?
}

/// Loads the command manifest for iOS (ADR-4: the palette must consume the
/// synced manifest, never a hand-copied Swift list — see `GrCommand`).
///
/// `.relay` mode has no reachable server, so the primary source is a
/// checked-in bundled snapshot (`CommandManifest.json`, a straight copy of
/// `web/src/lib/command-manifest.json` — regenerate via
/// `cp web/src/lib/command-manifest.json app/Tesela-iOS/Sources/Data/CommandManifest.json`,
/// verified by `scripts/check-command-manifest-drift.sh`). When an `.http`
/// backend is reachable, `fetchRemote` picks up anything added since this
/// build's snapshot was taken by hitting the same `GET /commands` route the
/// web/server layer already serves.
enum CommandManifestSource {
    /// Load the bundled checked-in manifest. Returns `[]` (never throws) if
    /// the resource is missing or malformed — callers treat that as "no
    /// commands available" rather than crashing the shell.
    static func loadBundled() -> [CommandManifestEntry] {
        guard let url = Bundle.main.url(forResource: "CommandManifest", withExtension: "json"),
              let data = try? Data(contentsOf: url) else {
            return []
        }
        return (try? JSONDecoder().decode([CommandManifestEntry].self, from: data)) ?? []
    }

    /// Refresh from `GET /commands` on the given `.http` backend. Throws on
    /// any network/decode failure — callers fall back to the bundled
    /// snapshot rather than surfacing an error to the user.
    static func fetchRemote(baseURL: URL) async throws -> [CommandManifestEntry] {
        var req = URLRequest(url: baseURL.appendingPathComponent("commands"))
        req.timeoutInterval = 8
        let (data, response) = try await URLSession.shared.data(for: req)
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            throw URLError(.badServerResponse)
        }
        return try JSONDecoder().decode([CommandManifestEntry].self, from: data)
    }
}
