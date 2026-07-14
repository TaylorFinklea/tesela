import UIKit

/// Lightweight `UIApplicationDelegate` adaptor for sync-durability
/// Phase 3a — the APNs RECEIVING end on iOS. SwiftUI's `@main App`
/// doesn't expose the `UIApplicationDelegate` hooks we need
/// (remote-notification registration + the silent-push entry point),
/// so we adapt it via `@UIApplicationDelegateAdaptor` on `TeselaApp`.
///
/// The relay SERVER (separate, future work) will send a
/// `content-available: 1` silent push when a new batch lands for the
/// group. The token-send-to-relay endpoint exists and IS called — see
/// `RelayTicker.tickOnce` (relay-scoped `registerDevice(apnsToken:)`,
/// keyed by `apnsRegistrationKey` so HA→CF migrations re-register with
/// the new relay). This delegate captures + exposes the token + logs
/// it; the actual POST lives in `RelayTicker`, not here, so the
/// delegate stays free of relay references. Enabling Push for the App
/// ID (`aps-environment` entitlement) is also a SEPARATE step, out of
/// scope here; without it registration fails at RUNTIME, but the
/// CODE must still build.
final class AppDelegate: NSObject, UIApplicationDelegate {
    /// Hex-encoded APNs device token from registration. Stored statically
    /// so the future relay-registration step (and any in-app debug
    /// surface) can read it without a relay reference. Cleared to nil
    /// on registration failure so a stale token from a prior install
    /// can't linger across a re-install / signing-identity change.
    static var deviceTokenHex: String? = nil

    /// Last APNs registration failure (nil = none / success). Surfaced in
    /// the Sync settings diagnostic so a token-capture failure is visible
    /// in-app without attaching Console.app.
    static var lastRegistrationError: String? = nil

    /// Launch: kick off APNs registration. The first launch on a fresh
    /// install prompts the user for notification permission; subsequent
    /// launches just request the token. We don't gate this on any
    /// pairing state — the token is useful even before the user has
    /// paired, and the server can ignore pushes for unknown groups.
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        UIApplication.shared.registerForRemoteNotifications()
        return true
    }

    /// APNs handed us a device token. Tokens arrive as 32 raw bytes;
    /// format as a lowercase hex string (the conventional wire format
    /// for token registration endpoints) and stash it for the
    /// relay-side `registerDevice(apnsToken:)` step called from
    /// `RelayTicker.tickOnce` (relay-scoped, keyed by
    /// `apnsRegistrationKey`). The endpoint exists (CF Worker
    /// `handleRegisterDevice` in `cloudflare-relay/src/handlers.ts`);
    /// the network call lives in `RelayTicker`, not here.
    func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        let hex = deviceToken.map { String(format: "%02x", $0) }.joined()
        Self.deviceTokenHex = hex
        Self.lastRegistrationError = nil
        print("[apns] device token \(hex)")
    }

    /// Registration failed. The most common cause in dev is that the
    /// App ID hasn't had Push Notifications enabled in the developer
    /// portal (no `aps-environment` entitlement on the provisioning
    /// profile) — iOS returns `unregistered` or `bad device token` in
    /// that case. The CODE still builds; this is a runtime-only gap
    /// that's resolved out of band by enabling Push for the App ID.
    /// We clear any stale `deviceTokenHex` from a prior install so the
    /// future relay registration step doesn't POST a token iOS no
    /// longer recognises.
    func application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        Self.deviceTokenHex = nil
        Self.lastRegistrationError = error.localizedDescription
        print("[apns] registration failed: \(error.localizedDescription)")
    }

    /// Silent-push entry (the relay server sends `content-available: 1`
    /// when a new batch lands for the group). iOS wakes the suspended
    /// app in the background; we run the catch-up on the main actor
    /// (`RelayTicker` is `@MainActor` — see `RelayTicker.swift`'s
    /// class-level annotation) and report the catch-up's real result.
    /// The process-wide shared ticker owns the only Loro handle for the
    /// active group, so a warm wake never opens a second engine on the
    /// foreground shell's physical root.
    func application(
        _ application: UIApplication,
        didReceiveRemoteNotification userInfo: [AnyHashable: Any],
        fetchCompletionHandler completionHandler: @escaping (UIBackgroundFetchResult) -> Void
    ) {
        Task { @MainActor in
            let outcome = await RelayTicker.shared.runBackgroundCatchup()
            completionHandler(Self.fetchResult(for: outcome))
        }
    }

    static func fetchResult(
        for outcome: BackgroundCatchupOutcome
    ) -> UIBackgroundFetchResult {
        switch outcome {
        case .completed(newData: true):
            return .newData
        case .completed(newData: false), .unavailable:
            return .noData
        case .failed:
            return .failed
        }
    }
}
