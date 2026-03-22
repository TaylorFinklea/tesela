import Foundation
import Observation

// MARK: - Block (outliner block tree node)
@Observable
final class Block: Identifiable, @unchecked Sendable {
    let id: UUID
    var text: String
    var children: [Block]
    var indentLevel: Int
    var isCollapsed: Bool
    var todoState: TodoState?
    var tags: [String]
    var properties: [String: String]

    // First-class task properties (extracted from inline key:: value)
    var priority: Priority?
    var deadline: String?      // ISO date "2026-03-30"
    var scheduled: String?     // ISO date
    var effort: String?        // Duration "30m", "2h", "1d"

    init(
        id: UUID = UUID(),
        text: String,
        children: [Block] = [],
        indentLevel: Int = 0,
        isCollapsed: Bool = false,
        todoState: TodoState? = nil,
        tags: [String] = [],
        properties: [String: String] = [:]
    ) {
        self.id = id
        self.text = text
        self.children = children
        self.indentLevel = indentLevel
        self.isCollapsed = isCollapsed
        self.todoState = todoState
        self.tags = tags
        self.properties = properties
    }
}

// MARK: - TodoState
enum TodoState: String, Codable, Hashable, Sendable, CaseIterable {
    case todo = "TODO"
    case doing = "DOING"
    case done = "DONE"

    var next: TodoState {
        switch self {
        case .todo: .doing
        case .doing: .done
        case .done: .todo
        }
    }

    var displayChar: String {
        switch self {
        case .todo: "☐"
        case .doing: "◎"
        case .done: "☑"
        }
    }
}

// MARK: - Priority
enum Priority: String, Codable, Hashable, Sendable, CaseIterable {
    case critical = "critical"
    case high = "high"
    case medium = "medium"
    case low = "low"

    var displayChar: String {
        switch self {
        case .critical: "🔴"
        case .high: "🟠"
        case .medium: "🟡"
        case .low: "🔵"
        }
    }

    var next: Priority {
        switch self {
        case .critical: .high
        case .high: .medium
        case .medium: .low
        case .low: .critical
        }
    }
}
