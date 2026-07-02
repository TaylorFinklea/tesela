import Foundation
import Security

/// Keychain-backed store for the cached pairing code (tesela-tp0.2).
///
/// The pairing code carries the group id + symmetric group key (among
/// other pairing metadata) — see `decodePairingCode` /
/// `RelayTicker.buildCoordinator`. Before this cutover it lived in
/// plaintext `UserDefaults` under `"relay.cachedPairingCode"`, which is
/// readable by anything with app-sandbox filesystem access (device
/// backups, jailbreak tooling) — this is the E2E-crypto-adjacent gap
/// `tesela-tp0.2` closes on iOS. `RelayTicker` never persists the group
/// key through Rust/FFI (the FFI's `generate_group_identity` is a pure
/// function with no mosaic-root persistence), so this is the ENTIRE
/// iOS-side key storage surface — the desktop-side equivalent is
/// `crypto::keys::KeychainGroupKeyStore` in `tesela-sync`.
///
/// `kSecAttrAccessibleAfterFirstUnlock` (not `WhenUnlocked`) matches the
/// existing `UserDefaults` availability contract: the relay tick runs in
/// the background (scene phase `.active` on foreground, but a
/// background-fetch/push-triggered tick can run before the user has
/// unlocked the device this boot) and must be able to read the cached
/// code without requiring the device to be unlocked at that instant.
enum KeychainPairingCache {
    private static let service = "com.tesela.sync.pairing-code"
    private static let account = "relay.cachedPairingCode"

    /// Legacy plaintext key — pre-cutover installs cached the pairing
    /// code here. Read once for migration, then always deleted.
    private static let legacyDefaultsKey = "relay.cachedPairingCode"

    /// Load the cached pairing code, migrating a legacy plaintext
    /// `UserDefaults` value into the Keychain (and deleting the
    /// plaintext copy) the first time this runs post-cutover.
    static func load() -> String? {
        if let fromKeychain = read() {
            return fromKeychain
        }
        guard let legacy = UserDefaults.standard.string(forKey: legacyDefaultsKey) else {
            return nil
        }
        // The legacy plaintext is the ONLY copy of the pairing code (it
        // carries the group key): delete it ONLY after the Keychain write
        // verifiably succeeded, or a transient Keychain failure would
        // destroy the pairing entirely. Migration retries on next load.
        if write(legacy) {
            UserDefaults.standard.removeObject(forKey: legacyDefaultsKey)
        }
        return legacy
    }

    /// Persist a newly-cached/re-cached pairing code. Also clears any
    /// stale plaintext `UserDefaults` copy so a rotated/re-paired code
    /// can't leave the OLD code readable there.
    /// (Deleting the legacy plaintext here is safe regardless of the
    /// write outcome: it holds a PRIOR code, never the one being saved —
    /// the sole-copy hazard exists only on the `load()` migration path.)
    @discardableResult
    static func save(_ rawCode: String) -> Bool {
        let wrote = write(rawCode)
        UserDefaults.standard.removeObject(forKey: legacyDefaultsKey)
        return wrote
    }

    /// Drop the cached pairing code (invalidated on a definitive pairing
    /// failure — see `RelayTicker.ensureCoordinator`).
    static func clear() {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(query as CFDictionary)
        UserDefaults.standard.removeObject(forKey: legacyDefaultsKey)
    }

    private static func read() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }

    /// Returns whether the Keychain verifiably holds `value` afterward.
    @discardableResult
    private static func write(_ value: String) -> Bool {
        guard let data = value.data(using: .utf8) else { return false }
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        let attributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
        ]
        let status = SecItemCopyMatching(query as CFDictionary, nil)
        if status == errSecSuccess {
            return SecItemUpdate(query as CFDictionary, attributes as CFDictionary) == errSecSuccess
        } else {
            var addQuery = query
            addQuery.merge(attributes) { _, new in new }
            return SecItemAdd(addQuery as CFDictionary, nil) == errSecSuccess
        }
    }
}
