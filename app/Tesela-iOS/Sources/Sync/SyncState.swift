import SwiftUI
import Combine

/// Workspace-level sync state. Exposes the two flags the modified-marker
/// reads:
///   • `isReachable` — true when at least one peer is reachable
///   • `hasPendingEdits` — true when local edits haven't been seen by any peer yet
///
/// Per decision #13, the page-title `●` indicator renders only when
/// **both** `!isReachable && hasPendingEdits` are true. Continuous-save
/// is assumed invisible — the marker is a sync-state indicator, not a
/// file-write indicator.
///
/// For now the values are mocked via a debug toggle in Settings → Sync.
/// Phase 15 will hook them into the real Rust sync layer.
@MainActor
final class SyncState: ObservableObject {
    @Published var isReachable: Bool = true
    @Published var hasPendingEdits: Bool = false

    /// Drives the `●` indicator visibility.
    var showsModifiedMarker: Bool {
        !isReachable && hasPendingEdits
    }
}
