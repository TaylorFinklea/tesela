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

    func deepCopy() -> Block {
        let copy = Block(
            id: id,  // preserve identity for SwiftUI diffing
            text: text,
            children: children.map { $0.deepCopy() },
            indentLevel: indentLevel,
            isCollapsed: isCollapsed,
            tags: tags,
            properties: properties
        )
        copy.priority = priority
        copy.deadline = deadline
        copy.scheduled = scheduled
        copy.effort = effort
        return copy
    }

    // MARK: - Display text (clean text without tags or properties)

    /// First line of text with #tags stripped — what the user sees in the editor.
    var displayText: String {
        let firstLine = text.components(separatedBy: "\n").first ?? text
        // Strip only complete tags (that have a trailing space/punctuation).
        // Tags at end of line stay visible so user can see what they're typing.
        var result = firstLine
        for tag in BlockParser.extractTagsLive(from: firstLine) {
            result = result.replacingOccurrences(of: " #\(tag)", with: "")
            result = result.replacingOccurrences(of: "#\(tag) ", with: "")
            result = result.replacingOccurrences(of: "#\(tag)", with: "")
        }
        return result.trimmingCharacters(in: .whitespaces)
    }

    /// Update storage text from edited display text, preserving tags and property lines.
    func updateDisplayText(_ newDisplay: String) {
        let lines = text.components(separatedBy: "\n")
        let firstLine = lines.first ?? ""

        // Extract inline tags from original first line
        let tagPattern = try! NSRegularExpression(pattern: #"#[A-Za-z0-9_\-]+"#)
        let range = NSRange(firstLine.startIndex..., in: firstLine)
        let inlineTags = tagPattern.matches(in: firstLine, range: range)
            .compactMap { Range($0.range, in: firstLine).map { String(firstLine[$0]) } }

        // Rebuild: new display text + original tags + original property lines
        var result = newDisplay
        if !inlineTags.isEmpty {
            result += " " + inlineTags.joined(separator: " ")
        }
        if lines.count > 1 {
            result += "\n" + lines.dropFirst().joined(separator: "\n")
        }
        text = result
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
