import SwiftUI

// MARK: - GraphView
// Phase 11.9 — Force-directed knowledge graph
// Placeholder for current phase

struct GraphView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        ContentUnavailableView(
            "Graph View",
            systemImage: "point.3.connected.trianglepath.dotted",
            description: Text("Knowledge graph — coming in Phase 11.9")
        )
    }
}
