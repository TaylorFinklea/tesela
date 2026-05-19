import Foundation
import Combine

/// Device-local store of `MosaicProfile`s plus which one is currently
/// active. Persists to UserDefaults as JSON so the list survives
/// relaunch. When the active profile changes, observers can read
/// `activeProfile?.serverURL` to know which backend to attach to.
final class MosaicRegistry: ObservableObject {
    @Published private(set) var profiles: [MosaicProfile] = []
    @Published private(set) var activeID: UUID? = nil

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
            activeID = profile.id
        }
        persist()
    }

    func update(_ profile: MosaicProfile) {
        guard let idx = profiles.firstIndex(where: { $0.id == profile.id }) else { return }
        profiles[idx] = profile
        persist()
    }

    func delete(_ id: UUID) {
        profiles.removeAll { $0.id == id }
        if activeID == id {
            activeID = profiles.first?.id
        }
        persist()
    }

    func setActive(_ id: UUID) {
        guard profiles.contains(where: { $0.id == id }) else { return }
        activeID = id
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
