import SwiftUI
import BackgroundTasks

private final class BackgroundTaskCompletionOnce: @unchecked Sendable {
    private let lock = NSLock()
    private var completed = false

    func claim() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard !completed else { return false }
        completed = true
        return true
    }
}

@main
struct TeselaApp: App {
    /// Shell selection. The Graphite redesign is now the DEFAULT — it owns
    /// the daily-driver views and the collaborative-editing path, which the
    /// legacy `AppShell` does not have. The legacy shell is kept as an escape
    /// hatch behind the `-legacy` launch argument (Xcode scheme / `simctl
    /// launch … -legacy`) or the `tesela.useLegacyShell` default, until it's
    /// removed at the redesign cutover.
    private var useLegacyShell: Bool {
        ProcessInfo.processInfo.arguments.contains("-legacy")
            || UserDefaults.standard.bool(forKey: "tesela.useLegacyShell")
    }

    /// Single source of truth for the BGProcessingTask identifier — MUST
    /// match `BGTaskSchedulerPermittedIdentifiers` in project.yml AND the
    /// `BGProcessingTaskRequest(identifier:)` in `scheduleCatchup()`. If
    /// any of the three drift, iOS drops `submit()` with no on-device log.
    static let catchupTaskIdentifier = "app.tesela.ios.relay-catchup"

    /// Holds the in-flight background-catchup Task so the BGProcessingTask's
    /// `expirationHandler` can cancel it when iOS reclaims our background
    /// time. Only one catch-up runs at a time — a second launch handler
    /// invocation replaces the prior reference, but the prior Task is
    /// already completing on its own; the race is benign.
    private static var catchupTask: Task<Void, Never>? = nil

    /// Sync-durability Phase 3a — APNs RECEIVING end. The SwiftUI `@main`
    /// App doesn't expose `UIApplicationDelegate` hooks (remote-
    /// notification registration + the silent-push entry point), so we
    /// adapt `AppDelegate` here. The adaptor fires the delegate's
    /// `application(_:didFinishLaunchingWithOptions:)` before the rest
    /// of `init()` returns, which registers for remote notifications
    /// early enough that the first push can land on a cold install.
    /// See `AppDelegate.swift` for the full rationale + the token
    /// capture (the relay-registration endpoint doesn't exist yet).
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

    init() {
        // Register the BGProcessingTask handler BEFORE the app finishes
        // launching — iOS throws from `register` if it's called after
        // launch completion, and we lose the wake entirely. Use a
        // force-cast in the closure (`task as! BGProcessingTask`) because
        // `register(forTaskWithIdentifier:using:launchHandler:)` hands
        // back a `BGTask` and our handler type-narrows it for the
        // `setTaskCompleted` / `expirationHandler` surface it needs.
        BGTaskScheduler.shared.register(
            forTaskWithIdentifier: Self.catchupTaskIdentifier,
            using: nil
        ) { task in
            Self.handleCatchup(task as! BGProcessingTask)
        }
    }

    /// Body-task handler for the relay catch-up BGProcessingTask. Order
    /// matters: reschedule the NEXT request first (so the reschedule
    /// isn't blocked by this run's quota / network conditions), then
    /// run the work, and only `setTaskCompleted` AFTER the work
    /// returns. `expirationHandler` cancels the in-flight Task so a
    /// long-running catch-up yields cleanly when iOS reclaims time.
    static func handleCatchup(_ task: BGProcessingTask) {
        // 1. Reschedule the next request up-front. iOS dedupes the
        //    identifier, so this won't stack — the new request
        //    supersedes any pending one.
        scheduleCatchup()
        // 2. Wrap the async work in a Task we can cancel.
        let completion = BackgroundTaskCompletionOnce()
        let work = Task { @MainActor in
            let outcome = await RelayTicker.shared.runBackgroundCatchup()
            Self.catchupTask = nil
            guard !Task.isCancelled, completion.claim() else { return }
            task.setTaskCompleted(success: outcome.didRunSuccessfully)
        }
        Self.catchupTask = work
        // 3. If iOS reclaims our time, cancel the in-flight Task and
        //    mark the run failed. setTaskCompleted MUST be called
        //    from the expiration handler — iOS logs a watchdog trip
        //    otherwise.
        task.expirationHandler = {
            work.cancel()
            Self.catchupTask = nil
            if completion.claim() {
                task.setTaskCompleted(success: false)
            }
        }
    }

    /// Submit a BGProcessingTaskRequest to iOS so the next catch-up is
    /// queued. Safe to call repeatedly — iOS replaces the pending
    /// request for the same identifier. `requiresNetworkConnectivity`
    /// keeps us from burning battery on offline wakes; `earliestBeginDate`
    /// is a HINT, not a deadline — iOS still runs opportunistically
    /// (typically within 15–60 min when conditions are good, longer
    /// otherwise). `try?` swallows the "no permitted identifier"
    /// failure during dev (e.g. running a build whose Info.plist
    /// doesn't list the id) so the app doesn't crash on submit.
    static func scheduleCatchup() {
        let request = BGProcessingTaskRequest(identifier: catchupTaskIdentifier)
        request.requiresNetworkConnectivity = true
        request.requiresExternalPower = false
        request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        try? BGTaskScheduler.shared.submit(request)
    }

    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            if useLegacyShell {
                AppShell()
            } else {
                GrAppShell()
            }
        }
        .onChange(of: scenePhase) { _, newPhase in
            // Re-queue a catch-up whenever the app goes to background.
            // The foreground tick loop (`RelayTicker.start()`) only runs
            // while the app is active; once iOS suspends us the loop is
            // dead. A pending BGProcessingTask is the only way a
            // suspended device catches up at all. The handler reschedules
            // the next request up-front, so even if iOS runs THIS request
            // within minutes, the next one is already in the queue.
            if newPhase == .background {
                Self.scheduleCatchup()
            }
        }
    }
}
