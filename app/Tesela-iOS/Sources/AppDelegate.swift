import UIKit

/// Lightweight `UIApplicationDelegate` adaptor for sync-durability
/// Phase 3a — the APNs RECEIVING end on iOS. SwiftUI's `@main App`
/// doesn't expose the `UIApplicationDelegate` hooks we need
/// (remote-notification registration + the silent-push entry point),
/// so we adapt it via `@UIApplicationDelegateAdaptor` on `TeselaApp`.
///
/// The relay SERVER (separate, future work) will send a
/// `content-available: 1` silent push when a new batch lands for the
/// group. The token-send-to-relay endpoint does NOT exist yet — this
/// delegate captures + exposes the token + logs it; it does NOT
/// invent a network call. Enabling Push for the App ID
/// (`aps-environment` entitlement) is also a SEPARATE step, out of
/// scope here; without it registration fails at RUNTIME, but the
/// CODE must still build.
final class AppDelegate: NSObject, UIApplicationDelegate {
    /// Hex-encoded APNs device token from registration. Stored statically
    /// so the future relay-registration step (and any in-app debug
    /// surface) can read it without a relay reference. Cleared to nil
    /// on registration failure so a stale token from a prior install
    /// can't linger across a re-install / signing-identity change.
    static var deviceTokenHex: String? = nil

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
    /// for token registration endpoints) and stash it for the future
    /// relay-side `POST /relay/devices` step. No network call here —
    /// the relay registration endpoint doesn't exist yet.
    func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        let hex = deviceToken.map { String(format: "%02x", $0) }.joined()
        Self.deviceTokenHex = hex
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
        print("[apns] registration failed: \(error.localizedDescription)")
    }

    /// Silent-push entry (the relay server sends `content-available: 1`
    /// when a new batch lands for the group). iOS wakes the suspended
    /// app in the background; we run the catch-up on the main actor
    /// (`RelayTicker` is `@MainActor` — see `RelayTicker.swift`'s
    /// class-level annotation) and report `.newData` so iOS
    /// prioritises the next delivery window for our bundle id. The
    /// catch-up itself is a `RelayTicker().runBackgroundCatchup()` on
    /// a FRESH ticker (matching the same pattern the BGProcessingTask
    /// path uses in `TeselaApp.handleCatchup`): the live foreground
    /// `RelayTicker` belongs to `GrAppShell` and is not in scope here.
    func application(
        _ application: UIApplication,
        didReceiveRemoteNotification userInfo: [AnyHashable: Any],
        fetchCompletionHandler completionHandler: @escaping (UIBackgroundFetchResult) -> Void
    ) {
        Task { @MainActor in
            let ticker = RelayTicker()
            await ticker.runBackgroundCatchup()
            completionHandler(.newData)
        }
    }
}
