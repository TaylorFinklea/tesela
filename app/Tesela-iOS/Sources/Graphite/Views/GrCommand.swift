import Foundation

/// One executable command in the iOS command palette — the mobile stand-in
/// for the desktop `:` ex-mode / leader chords (touch can't do chords, so one
/// searchable list reaches every command).
///
/// ADR-4 (tesela-cib): the palette CONSUMES the shared command manifest
/// (tesela-cmdd.2, `CommandManifestSource`) instead of a hand-copied Swift
/// list — `palette(from:)` joins the live manifest against `executableIds`,
/// a NATIVE executor map keyed by stable manifest id. `GrAppShell.runCommand`
/// executes by that same id. A manifest entry absent from `executableIds`
/// has no iOS equivalent (pane splits, editor slash-insert verbs, desktop-
/// only navigation, …) and is simply not offered — growing palette coverage
/// means adding an id here (and a matching case in `runCommand`), never
/// inventing a new label/hint.
struct GrCommand: Identifiable, Equatable, Hashable {
    let id: String
    let label: String
    let hint: String
    let glyph: String
    let keywords: [String]

    /// Manifest ids this iOS build knows how to run natively.
    static let executableIds: Set<String> = [
        "daily", "agenda", "inbox",
        "settings-general", "settings-devices", "settings-sync",
        "settings-mosaic", "settings-data",
    ]

    /// Build the palette's offering: every manifest entry visible on the
    /// `palette` surface AND runnable on this shell (`executableIds`).
    static func palette(from manifest: [CommandManifestEntry]) -> [GrCommand] {
        manifest
            .filter { $0.surfaces.contains("palette") && executableIds.contains($0.id) }
            .map {
                GrCommand(
                    id: $0.id,
                    label: $0.label,
                    hint: $0.category.capitalized,
                    glyph: $0.glyph,
                    keywords: $0.keywords
                )
            }
    }

    /// Fuzzy-ish filter over label + hint + keywords; empty query → `commands`.
    static func matching(_ query: String, in commands: [GrCommand]) -> [GrCommand] {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return commands }
        return commands.filter { cmd in
            cmd.label.lowercased().contains(q)
                || cmd.hint.lowercased().contains(q)
                || cmd.keywords.contains { $0.lowercased().contains(q) }
        }
    }
}
