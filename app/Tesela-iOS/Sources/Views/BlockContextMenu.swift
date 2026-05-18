import SwiftUI

/// Native iOS context menu for a block. Long-press on a `BlockRow`
/// reveals these actions. Each row shows the friendly label as primary
/// text and the equivalent web verb as monospace secondary text — so
/// both audiences (Taylor, and external users) are served. Matches the
/// canvas's T-X3 long-press sheet.
///
/// Apply via `.contextMenu { BlockContextMenu(blockId: ...) }` on the
/// BlockRow content.
struct BlockContextMenu: View {
    let blockId: String
    var onAction: (BlockAction) -> Void = { _ in }

    var body: some View {
        // SwiftUI Menus auto-style native chrome.
        Section {
            Button {
                onAction(.edit)
            } label: {
                Label("Edit", systemImage: "pencil")
            }
            Button {
                onAction(.promote)
            } label: {
                Label("Promote to page", systemImage: "arrow.up.right")
            }
            Button {
                onAction(.convertToTag)
            } label: {
                Label("Convert to tag", systemImage: "number")
            }
        }
        Section {
            Button {
                onAction(.indent)
            } label: {
                Label("Indent", systemImage: "increase.indent")
            }
            Button {
                onAction(.moveTo)
            } label: {
                Label("Move to…", systemImage: "rectangle.portrait.and.arrow.right")
            }
            Button {
                onAction(.yankLink)
            } label: {
                Label("Copy block link", systemImage: "link")
            }
        }
        Section {
            Button {
                onAction(.archive)
            } label: {
                Label("Archive", systemImage: "archivebox")
            }
            Button(role: .destructive) {
                onAction(.delete)
            } label: {
                Label("Delete", systemImage: "trash")
            }
        }
    }
}

enum BlockAction {
    case edit
    case promote
    case convertToTag
    case indent
    case moveTo
    case yankLink
    case archive
    case delete

    /// The equivalent web verb name (mono caption hint when needed).
    var verb: String {
        switch self {
        case .edit:         return ":edit"
        case .promote:      return ":promote"
        case .convertToTag: return ":convert-to-tag"
        case .indent:       return ":indent"
        case .moveTo:       return ":move"
        case .yankLink:     return ":yank-link"
        case .archive:      return ":archive"
        case .delete:       return ":delete"
        }
    }
}
