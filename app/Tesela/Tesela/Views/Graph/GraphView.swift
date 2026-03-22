import SwiftUI

// MARK: - GraphView
// Force-directed knowledge graph. Nodes = pages, edges = wiki-links.

struct GraphView: View {
    @Environment(AppState.self) private var appState
    @State private var nodes: [GraphNode] = []
    @State private var edges: [EdgePair] = []
    @State private var timer: Timer?
    @State private var scale: CGFloat = 1.0
    @State private var offset: CGSize = .zero
    @State private var isSimulating = true

    var body: some View {
        GeometryReader { geo in
            Canvas { context, size in
                let center = CGPoint(x: size.width / 2 + offset.width,
                                     y: size.height / 2 + offset.height)

                // Draw edges
                for edge in edges {
                    guard let src = nodes.first(where: { $0.id == edge.source }),
                          let tgt = nodes.first(where: { $0.id == edge.target }) else { continue }
                    let from = CGPoint(x: center.x + src.x * scale, y: center.y + src.y * scale)
                    let to = CGPoint(x: center.x + tgt.x * scale, y: center.y + tgt.y * scale)
                    var path = Path()
                    path.move(to: from)
                    path.addLine(to: to)
                    context.stroke(path, with: .color(.secondary.opacity(0.3)), lineWidth: 1)
                }

                // Draw nodes
                for node in nodes {
                    let pos = CGPoint(x: center.x + node.x * scale, y: center.y + node.y * scale)
                    let radius = max(4, min(CGFloat(node.connections) * 2 + 4, 16)) * scale
                    let rect = CGRect(x: pos.x - radius, y: pos.y - radius,
                                      width: radius * 2, height: radius * 2)
                    context.fill(Circle().path(in: rect), with: .color(.accentColor))

                    // Label
                    let label = Text(node.title).font(.system(size: max(9, 11 * scale)))
                        .foregroundColor(.primary)
                    context.draw(label, at: CGPoint(x: pos.x, y: pos.y + radius + 8 * scale))
                }
            }
            .gesture(
                MagnifyGesture()
                    .onChanged { value in
                        scale = max(0.3, min(3.0, value.magnification))
                    }
            )
            .gesture(
                DragGesture()
                    .onChanged { value in
                        offset = value.translation
                    }
            )
            .onTapGesture { location in
                handleTap(at: location, in: geo.size)
            }
        }
        .task { await loadGraph() }
        .onDisappear { stopSimulation() }
    }

    // MARK: - Data loading

    private func loadGraph() async {
        let pages = appState.pages
        let serverEdges = (try? await appState.api.getAllEdges()) ?? []

        // Build node list
        var connectionCounts: [String: Int] = [:]
        for edge in serverEdges {
            connectionCounts[edge.source, default: 0] += 1
            connectionCounts[edge.target, default: 0] += 1
        }

        var newNodes: [GraphNode] = []
        for (i, page) in pages.enumerated() {
            // Distribute in a circle initially
            let angle = Double(i) / Double(max(pages.count, 1)) * 2 * .pi
            let radius: Double = 150
            newNodes.append(GraphNode(
                id: page.id,
                title: page.title,
                x: cos(angle) * radius,
                y: sin(angle) * radius,
                vx: 0, vy: 0,
                connections: connectionCounts[page.id] ?? 0
            ))
        }

        nodes = newNodes
        edges = serverEdges.map { EdgePair(source: $0.source, target: $0.target) }

        startSimulation()
    }

    // MARK: - Force simulation

    private func startSimulation() {
        isSimulating = true
        timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { _ in
            Task { @MainActor in stepSimulation() }
        }
    }

    private func stopSimulation() {
        timer?.invalidate()
        timer = nil
        isSimulating = false
    }

    private func stepSimulation() {
        guard isSimulating, !nodes.isEmpty else { return }
        let damping = 0.92
        let repulsion: Double = 3000
        let attraction: Double = 0.01
        let centerPull: Double = 0.005
        var totalVelocity: Double = 0

        // Repulsion between all pairs
        for i in 0..<nodes.count {
            for j in (i+1)..<nodes.count {
                let dx = nodes[i].x - nodes[j].x
                let dy = nodes[i].y - nodes[j].y
                let dist = max(sqrt(dx * dx + dy * dy), 1)
                let force = repulsion / (dist * dist)
                let fx = (dx / dist) * force
                let fy = (dy / dist) * force
                nodes[i].vx += fx
                nodes[i].vy += fy
                nodes[j].vx -= fx
                nodes[j].vy -= fy
            }
        }

        // Attraction along edges
        for edge in edges {
            guard let si = nodes.firstIndex(where: { $0.id == edge.source }),
                  let ti = nodes.firstIndex(where: { $0.id == edge.target }) else { continue }
            let dx = nodes[ti].x - nodes[si].x
            let dy = nodes[ti].y - nodes[si].y
            let fx = dx * attraction
            let fy = dy * attraction
            nodes[si].vx += fx
            nodes[si].vy += fy
            nodes[ti].vx -= fx
            nodes[ti].vy -= fy
        }

        // Center pull + apply velocity
        for i in 0..<nodes.count {
            nodes[i].vx -= nodes[i].x * centerPull
            nodes[i].vy -= nodes[i].y * centerPull
            nodes[i].vx *= damping
            nodes[i].vy *= damping
            nodes[i].x += nodes[i].vx
            nodes[i].y += nodes[i].vy
            totalVelocity += abs(nodes[i].vx) + abs(nodes[i].vy)
        }

        // Stop when settled
        if totalVelocity < 0.5 {
            stopSimulation()
        }
    }

    // MARK: - Interaction

    private func handleTap(at location: CGPoint, in size: CGSize) {
        let center = CGPoint(x: size.width / 2 + offset.width,
                             y: size.height / 2 + offset.height)
        let tapRadius: CGFloat = 20

        for node in nodes {
            let pos = CGPoint(x: center.x + node.x * scale, y: center.y + node.y * scale)
            let dist = hypot(location.x - pos.x, location.y - pos.y)
            if dist < tapRadius {
                if let page = appState.pages.first(where: { $0.id == node.id }) {
                    appState.open(page)
                }
                return
            }
        }
    }
}

// MARK: - Graph data types

struct GraphNode: Identifiable {
    let id: String
    let title: String
    var x: Double
    var y: Double
    var vx: Double
    var vy: Double
    var connections: Int
}

struct EdgePair: Hashable {
    let source: String
    let target: String
}
