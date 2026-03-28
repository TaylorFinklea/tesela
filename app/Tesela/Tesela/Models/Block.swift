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

    /// First line of text with ALL #tags stripped — backward compat default.
    var displayText: String {
        displayText(strippingOnly: nil)
    }

    /// First line of text with only the specified tags stripped.
    /// Pass a set of lowercased tag names to strip (type tags). Casual tags stay inline.
    /// Pass nil to strip ALL tags (legacy behavior).
    func displayText(strippingOnly typeTagNames: Set<String>?) -> String {
        let firstLine = text.components(separatedBy: "\n").first ?? text
        let liveTags = BlockParser.extractTagsLive(from: firstLine)
        let tagsToStrip: [String]
        if let typeTagNames {
            tagsToStrip = liveTags.filter { typeTagNames.contains($0.lowercased()) }
        } else {
            tagsToStrip = liveTags
        }
        var result = firstLine
        for tag in tagsToStrip {
            result = result.replacingOccurrences(of: " #\(tag)", with: "")
            result = result.replacingOccurrences(of: "#\(tag) ", with: "")
            result = result.replacingOccurrences(of: "#\(tag)", with: "")
        }
        return result.trimmingCharacters(in: .whitespaces)
    }

    /// Update storage text from edited display text, preserving hidden tags and property lines.
    /// Only type tags (in typeTagNames) are re-appended since casual tags remain in display text.
    func updateDisplayText(_ newDisplay: String, typeTagNames: Set<String>? = nil) {
        let lines = text.components(separatedBy: "\n")
        let firstLine = lines.first ?? ""

        // Extract COMPLETE tags from original text (not partial mid-typing ones)
        let originalTags = BlockParser.extractTagsLive(from: firstLine)

        // Only re-append type tags (the ones stripped from display), not casual tags
        let tagsToRestore: [String]
        if let typeTagNames {
            tagsToRestore = originalTags.filter { typeTagNames.contains($0.lowercased()) }
        } else {
            tagsToRestore = originalTags
        }
        let hashTags = tagsToRestore.map { "#\($0)" }

        var result = newDisplay
        let toAppend = hashTags.filter { !newDisplay.contains($0) }
        if !toAppend.isEmpty {
            result += " " + toAppend.joined(separator: " ")
        }

        // Append property continuation lines
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
