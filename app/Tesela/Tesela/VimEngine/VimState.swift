import Foundation

// MARK: - VimMode
enum VimMode: Equatable, Sendable {
    case normal
    case insert
    case visual
    case visualLine
    case operatorPending(Operator)

    var displayName: String {
        switch self {
        case .normal: "NORMAL"
        case .insert: "INSERT"
        case .visual: "VISUAL"
        case .visualLine: "VISUAL LINE"
        case .operatorPending: "OPERATOR"
        }
    }
}

// MARK: - Operator
enum Operator: Equatable, Sendable {
    case delete
    case change
    case yank
}

// MARK: - VimState
struct VimState: Sendable {
    var mode: VimMode = .normal
    var count: Int?                    // accumulated digit prefix
    var pendingCount: Int = 1          // count preserved through operator-pending
    var yank: String = ""              // last yanked text
    var lastEditCommand: EditorCommand? // for dot-repeat
    var searchQuery: String = ""

    mutating func appendCount(digit: Int) {
        count = (count ?? 0) * 10 + digit
    }

    mutating func resetCount() {
        count = nil
    }

    var effectiveCount: Int {
        count ?? 1
    }
}
