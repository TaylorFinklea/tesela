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
        tags: [String] = [],
        properties: [String: String] = [:]
    ) {
        self.id = id
        self.text = text
        self.children = children
        self.indentLevel = indentLevel
        self.isCollapsed = isCollapsed
        self.tags = tags
        self.properties = properties
    }

    // MARK: - Task computed properties

    var isTask: Bool { tags.contains("Task") }

    var status: String? {
        get { properties["status"] }
        set {
            if let v = newValue {
                properties["status"] = v
            } else {
                properties.removeValue(forKey: "status")
            }
        }
    }

    var statusDisplayChar: String {
        switch status {
        case "todo":  return "☐"
        case "doing": return "◎"
        case "done":  return "☑"
        default:      return "•"
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
