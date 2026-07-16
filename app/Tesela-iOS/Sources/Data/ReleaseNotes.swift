import Combine
import Foundation

enum ReleaseNotesPlatform: String, Codable, Hashable {
    case web
    case desktop
    case ios
}

struct ReleaseNotesCurrent: Decodable, Hashable {
    let web: String
    let desktop: String
    let ios: String

    func id(for platform: ReleaseNotesPlatform) -> String {
        switch platform {
        case .web: return web
        case .desktop: return desktop
        case .ios: return ios
        }
    }
}

struct ReleaseNotesIOSVersion: Decodable, Hashable {
    let marketing: String
    let build: String
}

struct ReleaseNotesVersions: Decodable, Hashable {
    let desktop: String?
    let ios: ReleaseNotesIOSVersion?
}

struct ReleaseNote: Decodable, Identifiable, Hashable {
    let id: String
    let publishedAt: String
    let title: String
    let summary: String
    let platforms: [ReleaseNotesPlatform]
    let versions: ReleaseNotesVersions
    let newItems: [String]
    let fixed: [String]
    let important: [String]

    private enum CodingKeys: String, CodingKey {
        case id
        case publishedAt
        case title
        case summary
        case platforms
        case versions
        case newItems = "new"
        case fixed
        case important
    }

    var publishedDate: Date? {
        ISO8601DateFormatter().date(from: publishedAt)
    }

    func versionLabel(for platform: ReleaseNotesPlatform) -> String {
        switch platform {
        case .web:
            return "Tesela Web"
        case .desktop:
            return versions.desktop.map { "Tesela \($0)" } ?? "Tesela Desktop"
        case .ios:
            guard let ios = versions.ios else { return "Tesela for iPhone" }
            return ReleaseNotesAppVersion.displayName(
                marketing: ios.marketing,
                build: ios.build
            )
        }
    }
}

struct ReleaseNotesCatalog: Decodable, Hashable {
    let schemaVersion: Int
    let current: ReleaseNotesCurrent
    let releases: [ReleaseNote]

    func currentRelease(for platform: ReleaseNotesPlatform) -> ReleaseNote? {
        let id = current.id(for: platform)
        return releases.first { $0.id == id && $0.platforms.contains(platform) }
    }

    func history(for platform: ReleaseNotesPlatform) -> [ReleaseNote] {
        let applicable = releases.filter { $0.platforms.contains(platform) }
        guard let currentIndex = applicable.firstIndex(where: {
            $0.id == current.id(for: platform)
        }) else {
            return []
        }
        return Array(applicable[currentIndex...])
    }

    func shouldPresentCurrent(
        for platform: ReleaseNotesPlatform,
        lastSeen: String?
    ) -> Bool {
        let applicable = releases.filter { $0.platforms.contains(platform) }
        guard let currentIndex = applicable.firstIndex(where: {
            $0.id == current.id(for: platform)
        }) else {
            return false
        }
        guard let lastSeen else { return true }
        guard let seenIndex = applicable.firstIndex(where: { $0.id == lastSeen }) else {
            return true
        }
        return seenIndex > currentIndex
    }

    func validated() throws -> ReleaseNotesCatalog {
        guard schemaVersion == 1 else {
            throw ReleaseNotesCatalogError.invalid("schemaVersion must be exactly 1")
        }
        guard !releases.isEmpty else {
            throw ReleaseNotesCatalogError.invalid("releases must not be empty")
        }

        var ids = Set<String>()
        var previousDate = Date.distantFuture
        for release in releases {
            guard !release.id.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                  !release.title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                  !release.summary.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                  let publishedDate = release.publishedDate else {
                throw ReleaseNotesCatalogError.invalid("release metadata is malformed")
            }
            guard ids.insert(release.id).inserted else {
                throw ReleaseNotesCatalogError.invalid("duplicate release id \(release.id)")
            }
            guard publishedDate < previousDate else {
                throw ReleaseNotesCatalogError.invalid("releases must be newest-first")
            }
            previousDate = publishedDate
            guard !release.platforms.isEmpty,
                  Set(release.platforms).count == release.platforms.count else {
                throw ReleaseNotesCatalogError.invalid("release platforms are malformed")
            }
            let items = release.newItems + release.fixed + release.important
            guard !items.isEmpty,
                  items.allSatisfy({ !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }) else {
                throw ReleaseNotesCatalogError.invalid("release change items are malformed")
            }
            if release.platforms.contains(.desktop) {
                guard let desktop = release.versions.desktop,
                      !desktop.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                    throw ReleaseNotesCatalogError.invalid("desktop release version is missing")
                }
            }
            if release.platforms.contains(.ios) {
                guard let ios = release.versions.ios,
                      !ios.marketing.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                      !ios.build.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                    throw ReleaseNotesCatalogError.invalid("iOS release version is missing")
                }
            }
        }

        for platform in [ReleaseNotesPlatform.web, .desktop, .ios] {
            guard currentRelease(for: platform) != nil else {
                throw ReleaseNotesCatalogError.invalid(
                    "current.\(platform.rawValue) does not resolve"
                )
            }
        }
        return self
    }
}

enum ReleaseNotesCatalogError: Error, LocalizedError {
    case invalid(String)

    var errorDescription: String? {
        switch self {
        case .invalid(let message): return message
        }
    }
}

enum ReleaseNotesCatalogSource {
    static func decode(_ data: Data) throws -> ReleaseNotesCatalog {
        try JSONDecoder().decode(ReleaseNotesCatalog.self, from: data).validated()
    }

    static func loadBundled(bundle: Bundle = .main) -> ReleaseNotesCatalog? {
        guard let url = bundle.url(forResource: "ReleaseNotes", withExtension: "json"),
              let data = try? Data(contentsOf: url) else {
            return nil
        }
        do {
            return try decode(data)
        } catch {
            #if DEBUG
            print("Release notes unavailable: \(error.localizedDescription)")
            #endif
            return nil
        }
    }
}

struct ReleaseNotesPresentation: Identifiable {
    let catalog: ReleaseNotesCatalog?
    let platform: ReleaseNotesPlatform
    let current: ReleaseNote?

    var id: String { current?.id ?? "release-notes-unavailable-\(platform.rawValue)" }
    var history: [ReleaseNote] { catalog?.history(for: platform) ?? [] }
}

@MainActor
final class ReleaseNotesPresenter: ObservableObject {
    @Published var presentation: ReleaseNotesPresentation?

    let catalog: ReleaseNotesCatalog?
    let platform: ReleaseNotesPlatform
    let defaults: UserDefaults
    private var sessionSeen = Set<String>()

    init(
        catalog: ReleaseNotesCatalog? = ReleaseNotesCatalogSource.loadBundled(),
        platform: ReleaseNotesPlatform = .ios,
        defaults: UserDefaults = .standard
    ) {
        self.catalog = catalog
        self.platform = platform
        self.defaults = defaults
    }

    var seenKey: String {
        "releaseNotes.lastSeen.\(platform.rawValue)"
    }

    func autoPresentIfNeeded(onboardingComplete: Bool) {
        guard onboardingComplete,
              presentation == nil,
              let catalog,
              let current = catalog.currentRelease(for: platform) else {
            return
        }
        let sessionKey = "\(platform.rawValue):\(current.id)"
        guard !sessionSeen.contains(sessionKey),
              catalog.shouldPresentCurrent(
                for: platform,
                lastSeen: defaults.string(forKey: seenKey)
              ) else {
            return
        }
        presentation = ReleaseNotesPresentation(
            catalog: catalog,
            platform: platform,
            current: current
        )
    }

    func presentCurrent() {
        guard let catalog,
              let current = catalog.currentRelease(for: platform) else {
            presentation = ReleaseNotesPresentation(
                catalog: nil,
                platform: platform,
                current: nil
            )
            return
        }
        presentation = ReleaseNotesPresentation(
            catalog: catalog,
            platform: platform,
            current: current
        )
    }

    func markCurrentRendered() {
        guard let catalog,
              let current = catalog.currentRelease(for: platform) else {
            return
        }
        sessionSeen.insert("\(platform.rawValue):\(current.id)")
        defaults.set(current.id, forKey: seenKey)
    }
}

enum ReleaseNotesAppVersion {
    static func displayName(bundle: Bundle = .main) -> String {
        displayName(
            marketing: bundle.object(
                forInfoDictionaryKey: "CFBundleShortVersionString"
            ) as? String,
            build: bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String
        )
    }

    static func displayName(marketing: String?, build: String?) -> String {
        guard let marketing, !marketing.isEmpty else { return "Tesela" }
        guard let build, !build.isEmpty else { return "Tesela \(marketing)" }
        return "Tesela \(marketing) (\(build))"
    }
}
