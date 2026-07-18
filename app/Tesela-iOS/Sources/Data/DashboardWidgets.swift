import Foundation

enum DashboardWidgetIcon {
    static func normalized(_ value: String?) -> String {
        switch value?.lowercased() {
        case "task": return "square-check"
        case "project": return "folder"
        case "person": return "user"
        case "cal": return "calendar"
        case "note": return "file-text"
        case let known? where [
            "square-check", "folder", "user", "calendar", "file-text",
            "search", "inbox", "circle-dot", "clock", "star", "pin", "bolt",
        ].contains(known):
            return known
        default:
            return "search"
        }
    }
}

/// Device-local ordering for the Dashboard's in-app widgets. The source
/// identifiers are namespaced so a Query note and saved view can share an id
/// without colliding. Source definitions remain synced; only presentation
/// order/collapse state lives in UserDefaults.
struct DashboardWidgetPlacement: Codable, Equatable, Hashable, Identifiable {
    var id: String
    var fallbackTitle: String
    var collapsed: Bool

    var sourceID: String {
        guard let separator = id.firstIndex(of: ":") else { return id }
        return String(id[id.index(after: separator)...])
    }
}

struct DashboardWidgetLayout: Codable, Equatable {
    static let storageKey = "tesela.dashboard.widget-layout.v1"
    static let currentVersion = 1

    var version: Int
    var placements: [DashboardWidgetPlacement]

    static let defaultLayout = DashboardWidgetLayout(
        version: currentVersion,
        placements: [
            DashboardWidgetPlacement(id: "builtin:agenda", fallbackTitle: "Agenda", collapsed: false),
            DashboardWidgetPlacement(id: "builtin:inbox", fallbackTitle: "Inbox", collapsed: false),
            DashboardWidgetPlacement(id: "builtin:sync-health", fallbackTitle: "Sync Health", collapsed: false),
        ]
    )

    static func load(defaults: UserDefaults = .standard) -> DashboardWidgetLayout {
        guard let data = defaults.data(forKey: storageKey),
              let decoded = try? JSONDecoder().decode(DashboardWidgetLayout.self, from: data)
        else { return defaultLayout }
        return decoded.normalized()
    }

    func save(defaults: UserDefaults = .standard) {
        guard let data = try? JSONEncoder().encode(normalized()) else { return }
        defaults.set(data, forKey: Self.storageKey)
    }

    func normalized() -> DashboardWidgetLayout {
        guard version == Self.currentVersion else { return Self.defaultLayout }
        var seen = Set<String>()
        let normalized = placements.compactMap { placement -> DashboardWidgetPlacement? in
            guard Self.isStableID(placement.id), seen.insert(placement.id).inserted else { return nil }
            let title = placement.fallbackTitle.trimmingCharacters(in: .whitespacesAndNewlines)
            return DashboardWidgetPlacement(
                id: placement.id,
                fallbackTitle: title.isEmpty ? placement.sourceID : title,
                collapsed: placement.collapsed
            )
        }
        return DashboardWidgetLayout(version: Self.currentVersion, placements: normalized)
    }

    func adding(_ candidate: DashboardWidgetCandidate) -> DashboardWidgetLayout {
        guard !placements.contains(where: { $0.id == candidate.id }) else { return self }
        var copy = normalized()
        copy.placements.append(
            DashboardWidgetPlacement(id: candidate.id, fallbackTitle: candidate.title, collapsed: false)
        )
        return copy
    }

    func removing(_ id: String) -> DashboardWidgetLayout {
        var copy = normalized()
        copy.placements.removeAll { $0.id == id }
        return copy
    }

    func moving(_ id: String, by delta: Int) -> DashboardWidgetLayout {
        var copy = normalized()
        guard let index = copy.placements.firstIndex(where: { $0.id == id }) else { return copy }
        let destination = index + delta
        guard copy.placements.indices.contains(destination) else { return copy }
        copy.placements.swapAt(index, destination)
        return copy
    }

    func toggling(_ id: String) -> DashboardWidgetLayout {
        var copy = normalized()
        guard let index = copy.placements.firstIndex(where: { $0.id == id }) else { return copy }
        copy.placements[index].collapsed.toggle()
        return copy
    }

    private static func isStableID(_ id: String) -> Bool {
        ["builtin:", "query:", "view:"].contains { id.hasPrefix($0) && id.count > $0.count }
    }
}

struct DashboardWidgetCandidate: Equatable, Hashable, Identifiable {
    enum SourceKind: String {
        case builtin
        case query
        case view
    }

    let id: String
    let sourceKind: SourceKind
    let sourceID: String
    let title: String
    let subtitle: String
    let icon: String

    static let builtins = [
        DashboardWidgetCandidate(
            id: "builtin:agenda", sourceKind: .builtin, sourceID: "agenda",
            title: "Agenda", subtitle: "Open tasks and scheduled work", icon: "square-check"
        ),
        DashboardWidgetCandidate(
            id: "builtin:inbox", sourceKind: .builtin, sourceID: "inbox",
            title: "Inbox", subtitle: "Canonical saved view", icon: "inbox"
        ),
        DashboardWidgetCandidate(
            id: "builtin:sync-health", sourceKind: .builtin, sourceID: "sync-health",
            title: "Sync Health", subtitle: "This device's relay status", icon: "circle-dot"
        ),
    ]
}

/// A Query note projected into the Dashboard catalog. `revision` is the
/// canonical note modification revision supplied by the active backend.
struct DashboardQueryDefinition: Equatable, Hashable, Identifiable {
    let id: String
    let title: String
    let dsl: String
    let group: String?
    let sort: String?
    let icon: String
    let revision: String

    var placementID: String { "query:\(id)" }
}

struct DashboardQueryProjection: Equatable, Hashable, Identifiable {
    let id: String
    let title: String
    let dsl: String
    let group: String?
    let sort: String?
    let icon: String
    let definitionRevision: String

    init(query: DashboardQueryDefinition) {
        id = query.placementID
        title = query.title
        dsl = query.dsl
        group = query.group
        sort = query.sort
        icon = query.icon
        definitionRevision = query.revision
    }

    init(view: SavedView) {
        id = view.id == SavedView.builtinInboxId ? "builtin:inbox" : "view:\(view.id)"
        title = view.name
        dsl = view.dsl
        group = view.displayGroupBy
        sort = view.displayTableConfig?.sortBy.map {
            "\($0) \(view.displayTableConfig?.sortDir ?? "asc")"
        }
        icon = view.id == SavedView.builtinInboxId ? "inbox" : "search"
        definitionRevision = view.dashboardRevision
    }
}

extension DashboardQueryDefinition {
    var candidate: DashboardWidgetCandidate {
        DashboardWidgetCandidate(
            id: placementID,
            sourceKind: .query,
            sourceID: id,
            title: title,
            subtitle: "Query note",
            icon: icon
        )
    }
}

extension SavedView {
    var dashboardCandidate: DashboardWidgetCandidate {
        DashboardWidgetCandidate(
            id: id == Self.builtinInboxId ? "builtin:inbox" : "view:\(id)",
            sourceKind: id == Self.builtinInboxId ? .builtin : .view,
            sourceID: id,
            title: name,
            subtitle: id == Self.builtinInboxId ? "Canonical saved view" : "Saved view",
            icon: id == Self.builtinInboxId ? "inbox" : "search"
        )
    }

    /// Deterministic definition token used to restart a projection when the
    /// synced views registry changes. It deliberately avoids Swift's randomized
    /// `Hasher`, which is process-local rather than a canonical revision.
    var dashboardRevision: String {
        [
            id,
            name,
            dsl,
            String(order),
            String(builtin),
            displayMode,
            displayGroupBy ?? "",
            displayShowDone.map(String.init) ?? "",
            displayTableConfig?.hidden.joined(separator: ",") ?? "",
            displayTableConfig?.order.joined(separator: ",") ?? "",
            displayTableConfig?.sortBy ?? "",
            displayTableConfig?.sortDir ?? "",
        ].joined(separator: "\u{1F}")
    }
}
