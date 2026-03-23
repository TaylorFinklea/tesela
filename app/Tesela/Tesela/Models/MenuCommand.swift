import Foundation

// MARK: - MenuCommand
// Shared command definition used by both slash menu and space leader menu.

struct MenuCommand: Identifiable {
    let id: String
    let label: String
    let icon: String
    let shortcutHint: String?
    let category: String?
    let keywords: [String]  // for slash menu filtering

    init(id: String, label: String, icon: String, shortcutHint: String? = nil, category: String? = nil, keywords: [String] = []) {
        self.id = id
        self.label = label
        self.icon = icon
        self.shortcutHint = shortcutHint
        self.category = category
        self.keywords = keywords
    }
}

// MARK: - Command Registry
enum CommandRegistry {
    static let all: [MenuCommand] = [
        // Task
        MenuCommand(id: "todo",      label: "Toggle Todo",     icon: "☐", shortcutHint: "t",  category: "task", keywords: ["todo", "checkbox", "check"]),
        MenuCommand(id: "doing",     label: "Set Doing",       icon: "◎", shortcutHint: nil,  category: "task", keywords: ["doing", "progress", "wip"]),
        MenuCommand(id: "done",      label: "Set Done",        icon: "☑", shortcutHint: nil,  category: "task", keywords: ["done", "complete", "finish"]),
        MenuCommand(id: "deadline",  label: "Set Deadline",    icon: "⚑", shortcutHint: "⌘D", category: "task", keywords: ["deadline", "due", "date"]),
        MenuCommand(id: "scheduled", label: "Set Scheduled",   icon: "📅", shortcutHint: "⌘⇧D", category: "task", keywords: ["scheduled", "plan", "start"]),
        MenuCommand(id: "priority",  label: "Set Priority",    icon: "🟠", shortcutHint: nil,  category: "task", keywords: ["priority", "important", "urgent"]),
        MenuCommand(id: "effort",    label: "Set Effort",      icon: "⏱", shortcutHint: nil,  category: "task", keywords: ["effort", "time", "estimate", "duration"]),

        // Insert
        MenuCommand(id: "link",    label: "Insert Link",    icon: "🔗", category: "insert", keywords: ["link", "wiki", "reference", "page"]),
        MenuCommand(id: "tag",     label: "Insert Tag",     icon: "#",  category: "insert", keywords: ["tag", "hashtag", "label"]),

        // Block
        MenuCommand(id: "block-below",  label: "New Block Below",   icon: "↓", shortcutHint: "o",  category: "block", keywords: ["new", "block", "below"]),
        MenuCommand(id: "block-above",  label: "New Block Above",   icon: "↑", shortcutHint: "O",  category: "block", keywords: ["new", "block", "above"]),
        MenuCommand(id: "delete-block", label: "Delete Block",      icon: "✕", shortcutHint: "dd", category: "block", keywords: ["delete", "remove"]),
        MenuCommand(id: "indent",       label: "Indent Block",      icon: "→", shortcutHint: ">>", category: "block", keywords: ["indent", "nest"]),
        MenuCommand(id: "dedent",       label: "Dedent Block",      icon: "←", shortcutHint: "<<", category: "block", keywords: ["dedent", "outdent", "unnest"]),
    ]

    static func matching(query: String) -> [MenuCommand] {
        guard !query.isEmpty else { return all }
        let q = query.lowercased()
        return all.filter { cmd in
            cmd.label.localizedCaseInsensitiveContains(q) ||
            cmd.id.localizedCaseInsensitiveContains(q) ||
            cmd.keywords.contains { $0.hasPrefix(q) }
        }
    }

    // Space menu categories
    static let spaceCategories: [(key: String, label: String, icon: String)] = [
        ("t", "Task",   "☐"),
        ("b", "Block",  "▪"),
        ("f", "Find",   "🔍"),
        ("n", "Navigate", "→"),
    ]

    static func commandsForCategory(_ key: String) -> [(key: String, command: MenuCommand)] {
        switch key {
        case "t": return [
            ("t", all.first { $0.id == "todo" }!),
            ("d", all.first { $0.id == "deadline" }!),
            ("s", all.first { $0.id == "scheduled" }!),
            ("p", all.first { $0.id == "priority" }!),
            ("e", all.first { $0.id == "effort" }!),
        ]
        case "b": return [
            ("o", all.first { $0.id == "block-below" }!),
            ("O", all.first { $0.id == "block-above" }!),
            ("d", all.first { $0.id == "delete-block" }!),
            (">", all.first { $0.id == "indent" }!),
            ("<", all.first { $0.id == "dedent" }!),
        ]
        case "f": return [
            ("f", MenuCommand(id: "search", label: "Search Pages", icon: "🔍", category: "find")),
        ]
        case "n": return [
            ("g", MenuCommand(id: "nav-graph", label: "Graph View", icon: "◎", category: "nav")),
            ("t", MenuCommand(id: "nav-tiles", label: "Tiles View", icon: "📅", category: "nav")),
            ("p", MenuCommand(id: "nav-pages", label: "Pages View", icon: "📄", category: "nav")),
        ]
        default: return []
        }
    }
}
