import Foundation

/// A command in the iOS command palette — the mobile stand-in for the
/// desktop `:` ex-mode / leader chords (touch can't do chords, so one
/// searchable list reaches every command). Pure data; `GrAppShell.runCommand`
/// executes it by `id`.
struct GrCommand: Identifiable, Equatable, Hashable {
    let id: String
    let label: String
    let hint: String
    let icon: String   // SF Symbol

    /// The built-in catalog: navigation across the tabs + global actions
    /// the shell can run. (Insert verbs live in the editor's `/` slash menu;
    /// block actions live on the keyboard toolbar — this is for everything
    /// else.)
    static let catalog: [GrCommand] = [
        GrCommand(id: "goto.daily",     label: "Go to Daily",   hint: "Today's journal",          icon: "calendar"),
        GrCommand(id: "goto.agenda",    label: "Go to Agenda",  hint: "Scheduled + deadlines",    icon: "calendar.day.timeline.left"),
        GrCommand(id: "goto.inbox",     label: "Go to Inbox",   hint: "Untriaged + saved views",  icon: "tray"),
        GrCommand(id: "goto.library",   label: "Go to Library", hint: "All pages",                icon: "books.vertical"),
        GrCommand(id: "goto.search",    label: "Search",        hint: "Find pages, blocks, tags", icon: "magnifyingglass"),
        GrCommand(id: "action.refresh", label: "Sync now",      hint: "Pull the latest from the relay", icon: "arrow.triangle.2.circlepath"),
        GrCommand(id: "open.settings",  label: "Settings",      hint: "Backend, keyboard, sync",  icon: "gearshape"),
    ]

    /// Fuzzy-ish filter over label + hint; empty query → the whole catalog.
    static func matching(_ query: String) -> [GrCommand] {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return catalog }
        return catalog.filter {
            $0.label.lowercased().contains(q) || $0.hint.lowercased().contains(q)
        }
    }
}
