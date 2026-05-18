import SwiftUI

/// A single outliner block — one `- ` bullet on the web. Renders the
/// bullet, the body text (with inline wiki/bold), the trailing tag
/// chips, and respects indent depth. Tapping a task toggles its done
/// state via the optional `onToggleTask` closure.
struct BlockRow: View {
    /// Stable identifier (block id, position index, etc.).
    let id: String
    let kind: BlockKind
    let text: String
    var indent: Int = 0
    var isDone: Bool = false
    var tags: [String] = []
    var onToggleTask: (() -> Void)? = nil

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            bullet
            VStack(alignment: .leading, spacing: 4) {
                content
                if !tags.isEmpty {
                    HStack(spacing: 4) {
                        ForEach(tags, id: \.self) { tag in
                            TagChip(value: tag)
                        }
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.leading, CGFloat(18 + indent * 18))
        .padding(.trailing, 18)
        .padding(.vertical, 6)
        .contentShape(Rectangle())
        .onTapGesture {
            if kind == .task { onToggleTask?() }
        }
        .contextMenu {
            BlockContextMenu(blockId: id)
        }
    }

    // ── Bullet (task checkbox or project dot or note dot) ───────────────

    @ViewBuilder
    private var bullet: some View {
        switch kind {
        case .task:
            taskCheckbox
        case .project:
            Text("·")
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(theme.typeProject)
                .frame(width: 14, alignment: .center)
                .padding(.top, 2)
        default:
            Text("·")
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
                .frame(width: 14, alignment: .center)
                .padding(.top, 2)
        }
    }

    private var taskCheckbox: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 3)
                .stroke(theme.typeTask, lineWidth: 1.5)
                .frame(width: 14, height: 14)
            if isDone {
                RoundedRectangle(cornerRadius: 3)
                    .fill(theme.typeTask)
                    .frame(width: 14, height: 14)
                Icon(name: .check, size: 10, lineWidth: 2.5)
                    .foregroundStyle(theme.bg)
            }
        }
        .padding(.top, 4)
    }

    // ── Content (body text with inline parsing) ─────────────────────────

    private var contentColor: Color {
        isDone ? theme.fgSubtle : theme.fgDefault
    }

    private var content: some View {
        BlockText(text: text)
            .font(.system(size: 15))
            .foregroundStyle(contentColor)
            .strikethrough(isDone, color: theme.fgSubtle)
            .lineSpacing(3)
            .fixedSize(horizontal: false, vertical: true)
    }
}
